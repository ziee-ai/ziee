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

/// The agent-core-driven analog of `StreamingService::start_generation` (ITEM-24).
/// Does the fire-and-forget pre-loop MESSAGE-LIFECYCLE (provider + user/assistant
/// rows via the existing registry — DEC-22), then spawns `ChatAgentTurn` (which
/// streams tokens + persists blocks through the ports) seeded with `Resume` (the
/// user message is already persisted, so the loop LOADS it rather than re-appending).
/// Returns the persisted ids synchronously, exactly like the legacy path.
///
/// Wave-5 scope: no ported context-injector extensions yet, so `system` is empty
/// and `tool_scope` has no attached servers (a basic text turn). This is the path
/// the `ZIEE_CHAT_AGENT_CORE=1` flag routes to for behavioral verification.
pub async fn start_generation_agent_core(
    pool: sqlx::PgPool,
    registry: Option<Arc<ExtensionRegistry>>,
    branch_id: Uuid,
    conversation_id: Uuid,
    user_id: Uuid,
    request: crate::modules::chat::core::extension::SendMessageRequest,
) -> Result<(Option<Uuid>, Uuid), AppError> {
    use crate::core::Repos;
    use crate::modules::chat::core::ai_provider::create_provider_from_model_id;
    use crate::modules::chat::core::models::MessageRole;
    use crate::modules::chat::core::types::streaming::{
        SSEChatStreamErrorData, SSEChatStreamEvent, SSEChatStreamStartedData,
    };
    use crate::modules::chat::stream::{publish_frame, ChatStreamFrame};
    use crate::utils::cancellation::CANCELLATION_TRACKER;

    // Single-flight per conversation (same guard as the legacy path).
    if !crate::modules::chat::stream::begin_generation(conversation_id) {
        return Err(AppError::new(
            axum::http::StatusCode::CONFLICT,
            "GENERATION_IN_PROGRESS",
            "A reply is already being generated for this conversation",
        ));
    }

    // Everything from here to the spawn must release the slot on error.
    let setup = async {
        let (provider, model_name, ..) =
            create_provider_from_model_id(request.model_id, user_id).await?;

        // Conditionally create the user message (extensions may suppress it, e.g.
        // MCP tool-approval resumption).
        let preliminary_context = StreamContext {
            conversation_id,
            branch_id,
            message_id: None,
            user_id,
            pool: pool.clone(),
            metadata: std::collections::HashMap::new(),
            iteration: 0,
        };
        let should_create = registry
            .as_ref()
            .map(|r| r.should_create_user_message(&request))
            .unwrap_or(true);
        let user_message_id = if should_create {
            let extension_content = if let Some(r) = &registry {
                r.collect_user_message_content(&preliminary_context, &request, &request.content)
                    .await?
            } else {
                Vec::new()
            };
            let user_message = Repos
                .chat
                .core
                .create_message(branch_id, MessageRole::User.as_str(), Some(request.model_id))
                .await?;
            if let Some(r) = &registry {
                r.after_user_message_created(&preliminary_context, &user_message, &request)
                    .await?;
            }
            for (index, content_data) in extension_content.into_iter().enumerate() {
                Repos
                    .chat
                    .core
                    .create_content(
                        user_message.id,
                        &content_data.content_type(),
                        content_data,
                        index as i32,
                    )
                    .await?;
            }
            Some(user_message.id)
        } else {
            None
        };

        // Get or create the assistant message (resume reuses the existing one).
        let assistant_message_id = if let Some(r) = &registry {
            if let Some(id) = r.provide_assistant_message(&request, branch_id).await? {
                id
            } else {
                Repos
                    .chat
                    .core
                    .create_message(branch_id, MessageRole::Assistant.as_str(), None)
                    .await?
                    .id
            }
        } else {
            Repos
                .chat
                .core
                .create_message(branch_id, MessageRole::Assistant.as_str(), None)
                .await?
                .id
        };
        Ok::<_, AppError>((provider, model_name, user_message_id, assistant_message_id))
    }
    .await;

    let (provider, model_name, user_message_id, assistant_message_id) = match setup {
        Ok(v) => v,
        Err(e) => {
            crate::modules::chat::stream::end_generation(conversation_id);
            return Err(e);
        }
    };

    // Per-`assistant_message_id` stop-generation token (bridged into the crate loop).
    let cancel_token = CANCELLATION_TRACKER.create_token(assistant_message_id).await;
    let owner_id = user_id;

    // Opening `started` frame — seeds the message on receiving devices + opens the
    // replay buffer for mid-stream join (the sink emits Content/Complete after).
    publish_frame(
        owner_id,
        ChatStreamFrame::new(
            conversation_id,
            SSEChatStreamEvent::Started(SSEChatStreamStartedData {
                user_message_id,
                conversation_id,
                branch_id,
            }),
        ),
    );

    let is_resume = user_message_id.is_none();
    tokio::spawn(async move {
        let turn = ChatAgentTurn {
            pool,
            registry,
            user_id,
            conversation_id,
            branch_id,
            assistant_message_id,
            provider,
            model_name,
            // No ported context extensions yet → no attached tool servers.
            tool_scope: ToolScope::default(),
            inputs: serde_json::Value::Null,
            cancel_token: cancel_token.clone(),
            sse_tx: None,
            extensions: vec![],
        };
        // The user message is already persisted, so LOAD it (Resume), don't re-append.
        let result = turn.run(TurnSeed::Resume).await;

        if let Err(e) = result {
            // The loop errored before emitting a terminal Complete frame — surface it
            // so the client's stream ends instead of hanging.
            publish_frame(
                owner_id,
                ChatStreamFrame::new(
                    conversation_id,
                    SSEChatStreamEvent::Error(SSEChatStreamErrorData {
                        message: e.to_string(),
                        code: Some("AGENT_LOOP_ERROR".into()),
                    }),
                ),
            );
        }

        CANCELLATION_TRACKER.remove_download(assistant_message_id).await;
        crate::modules::chat::stream::end_generation(conversation_id);

        // Notify the user's other surfaces to refetch the committed turn.
        crate::modules::sync::publish(
            crate::modules::sync::SyncEntity::Conversation,
            crate::modules::sync::SyncAction::Update,
            conversation_id,
            crate::modules::sync::Audience::owner(owner_id),
            None,
        );
    });

    let _ = is_resume;
    Ok((user_message_id, assistant_message_id))
}
