//! `ChatAgentTurn` — the chat host for the shared `agent_core::AgentCore` loop
//! (ITEM-24/25/26, full extension re-home). Assembles the six chat-flavored ports
//! + the core compaction extension into an `AgentCore` and runs ONE turn, mapping
//! the loop's events to chat's SSE stream (via `ChatEventSink`) and persisting the
//! assistant message as per-block rows (via `ChatTranscriptStore`).
//!
//! Wave-5 scope: this drives the loop + streaming + persistence + tool exec +
//! cross-request approval. The pre-loop MESSAGE-LIFECYCLE (create user/assistant
//! rows, `should_create_user_message`/`provide_assistant_message` resume) stays in
//! the caller (DEC-22). Context-injection (system prompt, tool attach, params) is
//! contributed by the ported `AgentExtension`s passed in `extensions`.
//!
//! # UX walk
//! A user's message drives a full agentic turn: the model streams tokens (live via
//! `ChatEventSink`), may call tools (via `ChatToolProvider`, gated by
//! `ChatApprovalPolicy`/`ChatHumanGate`), and the reply is persisted block-by-block
//! — identical to today's chat experience, now on the shared loop.
//!
//! # Infra-integration walk
//! Touches: the provider streaming seam (`ProviderModelClient`), the per-user SSE
//! registry (sink), block persistence (transcript), the MCP session + recording
//! chokepoint + cross-request approval (`tool_use_approvals`), and the chat
//! stop-generation cancel token (bridged into the crate's cooperative `CancelToken`).

use std::sync::Arc;

use agent_core::{
    AgentCore, AgentEvent, AgentTurnRequest, Budget, CancelToken, CompactionExtension, Compactor,
    ProviderModelClient, ProviderModelClientFactory, SandboxMode, SubagentLimits, ToolScope,
    TurnSeed,
};
use ai_providers::{ChatMessage, Provider};
use axum::response::sse::Event;
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::agent_host::event_sink::ChatEventSink;
use crate::modules::chat::agent_host::gate::ChatHumanGate;
use crate::modules::chat::agent_host::resolver::{ChatCancel, ChatModelResolver, ChatToolProvider};
use crate::modules::workflow::dispatch::CancelSignal;
use crate::modules::chat::agent_host::transcript::ChatTranscriptStore;
use crate::modules::chat::core::extension::{ExtensionRegistry, StreamContext};
use crate::utils::cancellation::CancellationToken;

/// Window-relative soft limit above which the core compaction extension fires.
/// High so a normal chat turn never summarizes (chat's own summarization extension
/// owns real compaction); the machinery is wired for parity with the workflow host.
const CHAT_COMPACTION_SOFT_LIMIT_TOKENS: usize = 200_000;

/// Failsafe iteration cap (chat's `SAFETY_MAX_ITERATIONS`); real per-turn limits
/// come from MCP settings / the approval gate.
const CHAT_SAFETY_MAX_ITERATIONS: u32 = 1000;

/// One chat assistant turn, executed on the shared agent loop. Constructed by the
/// caller AFTER the user + assistant message rows exist (DEC-22).
pub struct ChatAgentTurn {
    pub pool: sqlx::PgPool,
    /// The chat extension registry (used by the transcript for content conversion,
    /// and — later — the source of the ported context-injector extensions).
    pub registry: Option<Arc<ExtensionRegistry>>,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    /// The assistant DB message this turn's blocks accumulate into.
    pub assistant_message_id: Uuid,
    /// The resolved provider for the turn's model (built by the caller via
    /// `create_provider_from_model_id`, exactly as today).
    pub provider: Arc<Provider>,
    pub model_name: String,
    /// The tool servers this turn may call (built-in NAMES pushed by context
    /// extensions + the conversation's own MCP servers). Empty = no tools.
    pub tool_scope: ToolScope,
    /// The opaque per-turn input bag (chat's `SendMessageRequest.extensions`).
    pub inputs: serde_json::Value,
    /// The per-`assistant_message_id` stop-generation token (a clone).
    pub cancel_token: CancellationToken,
    /// SSE sink for the gate's approval-required frames (None in tests).
    pub sse_tx: Option<UnboundedSender<Result<Event, Infallible>>>,
    /// Ordered context-injection extensions (assistant system prompt, params, tool
    /// attach, memory, …). Empty in the minimal loop-verification path.
    pub extensions: Vec<Arc<dyn agent_core::AgentExtension>>,
}

impl ChatAgentTurn {
    /// Run the turn. `seed` = the new user message (fresh turn) or `Resume`
    /// (cross-request approval resume — do not re-append the user message).
    pub async fn run(self, seed: TurnSeed) -> Result<Vec<AgentEvent>, AppError> {
        let transform_context = StreamContext {
            conversation_id: self.conversation_id,
            branch_id: self.branch_id,
            message_id: Some(self.assistant_message_id),
            user_id: self.user_id,
            pool: self.pool.clone(),
            metadata: std::collections::HashMap::new(),
            iteration: 0,
        };

        let transcript = Arc::new(ChatTranscriptStore {
            pool: self.pool.clone(),
            branch_id: self.branch_id,
            assistant_message_id: self.assistant_message_id,
            registry: self.registry.clone(),
            transform_context,
        });
        let sink = Arc::new(ChatEventSink::new(
            self.user_id,
            self.conversation_id,
            self.branch_id,
            self.assistant_message_id,
        ));
        let tools = Arc::new(ChatToolProvider::new(
            self.user_id,
            Some(self.conversation_id),
            self.cancel_token.clone(),
        ));
        let gate = Arc::new(ChatHumanGate {
            user_id: self.user_id,
            conversation_id: self.conversation_id,
            branch_id: self.branch_id,
            assistant_message_id: self.assistant_message_id,
            tx: self.sse_tx.clone(),
        });
        // The approval policy is resolved from the conversation's live MCP settings.
        let policy = Arc::new(
            crate::modules::chat::agent_host::gate::resolve_chat_approval_policy(
                self.user_id,
                self.conversation_id,
                self.branch_id,
            )
            .await?,
        );

        let model_client = Arc::new(ProviderModelClient::new(self.provider.clone()));

        let mut extensions = self.extensions.clone();
        // Core compaction extension (parity with the workflow host).
        extensions.push(Arc::new(CompactionExtension::new(
            Compactor::new(
                model_client.clone(),
                self.model_name.clone(),
                CHAT_COMPACTION_SOFT_LIMIT_TOKENS,
            ),
            transcript.clone(),
            sink.clone(),
            self.assistant_message_id,
        )));

        let core = AgentCore {
            transcript: transcript.clone(),
            sink: sink.clone(),
            tools,
            gate,
            policy,
            models: Arc::new(ChatModelResolver),
            model: model_client,
            model_factory: Arc::new(ProviderModelClientFactory),
            extensions,
            // Chat drives approvals through the human gate, not the reviewer.
            reviewer: None,
            budget: Budget::new(CHAT_SAFETY_MAX_ITERATIONS, 100_000_000, 100_000_000),
            limits: SubagentLimits::default(),
            // Sandbox is approval metadata; chat's policy keys on approval mode, so
            // this is not enforcement here. Read-only w/ network is the safe carrier.
            sandbox: SandboxMode::ReadOnly { network: true },
            model_name: self.model_name.clone(),
        };

        let req = AgentTurnRequest {
            run_id: self.assistant_message_id,
            user_id: self.user_id,
            seed,
            system: vec![],
            tool_scope: self.tool_scope.clone(),
            start_iteration: 1,
            inputs: self.inputs.clone(),
        };

        // Bridge the chat stop-generation token (poll-only) into the crate's
        // cooperative `CancelToken`: a background poll flips the crate token when
        // the user hits stop. The bridge is aborted when the turn finishes.
        let crate_cancel = CancelToken::new();
        let bridge = {
            let ct = crate_cancel.clone();
            let signal = ChatCancel::new(self.cancel_token.clone());
            tokio::spawn(async move {
                signal.cancelled().await; // poll-loop on the chat stop token
                ct.cancel();
            })
        };
        let result = core.run(req, crate_cancel).await;
        bridge.abort();
        result
    }

    /// Convenience: a fresh turn seeded by the user's message text.
    pub async fn run_new_message(self, user_text: String) -> Result<Vec<AgentEvent>, AppError> {
        self.run(TurnSeed::NewMessage(ChatMessage::user(user_text)))
            .await
    }
}
