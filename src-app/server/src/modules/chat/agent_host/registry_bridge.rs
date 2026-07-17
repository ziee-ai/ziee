//! `RegistryBridge` — the single `AgentExtension` that re-homes chat's whole
//! context-injection layer onto the agent-core loop (ITEM-24/25/26).
//!
//! Rather than 14 hand-copied per-module ports (which would drift from the tested
//! originals and score worse on modularity/maintainability), this ONE bridge runs
//! the existing `ExtensionRegistry::call_before_llm_call` — i.e. EVERY chat
//! extension's real `before_llm_call` in registered order — inside the crate loop's
//! `before_model` hook, each iteration, exactly as the legacy loop did. So the
//! assistant/project system prompts, memory retrieval, the MCP tool-list gathering
//! (which sets `request.tools`), disabled-server filtering, and cross-request
//! approval-decision processing all run through their SAME code.
//!
//! What it deliberately does NOT bridge: `call_after_llm_call`. Chat's
//! `after_llm_call` drives tool EXECUTION + loop CONTINUATION (the MCP extension) —
//! those responsibilities now belong to the agent-core ports (`ChatToolProvider` +
//! `ChatApprovalPolicy` + `ChatHumanGate`) + the loop's native tool-detection. The
//! non-loop side-effects it also carries (title generation, memory extraction) are
//! handled separately (see the dispatcher) so they don't re-drive the loop.

use std::sync::Arc;

use agent_core::{AgentExtension, Flow};
use ai_providers::{ChatMessage, ChatRequest, ContentBlock};
use async_trait::async_trait;
use axum::response::sse::Event;
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ExtensionRegistry, SendMessageRequest, StreamContext,
};

/// Bridges the chat `ExtensionRegistry`'s `before_llm_call` fan-out into a single
/// agent-core `AgentExtension`. Ordered LAST-ish so the crate's own assemble (empty
/// system/tool_scope) runs first, then this mutates the request with the real chat
/// context + tools — matching the legacy "assemble history, then before_llm_call".
pub struct RegistryBridge {
    registry: Arc<ExtensionRegistry>,
    /// The per-turn stream context (interior-mutable: `before_model` is `&self`,
    /// but the registry needs `&mut StreamContext` + a per-iteration bump).
    ctx: Mutex<StreamContext>,
    /// The original send request (attach flags, tool_approvals, file_ids, …) the
    /// extensions read; owned for the turn.
    send_request: SendMessageRequest,
    /// SSE sink for extension-emitted frames (McpApprovalRequired / titleUpdated).
    tx: Option<UnboundedSender<Result<Event, Infallible>>>,
    /// Provider/model identity seeded into `ctx.metadata` each iteration (title,
    /// file, and other hooks read these — the legacy loop seeds the same keys).
    provider_type: String,
    model_name: String,
    model_id: uuid::Uuid,
    provider_id: uuid::Uuid,
}

impl RegistryBridge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: Arc<ExtensionRegistry>,
        ctx: StreamContext,
        send_request: SendMessageRequest,
        tx: Option<UnboundedSender<Result<Event, Infallible>>>,
        provider_type: String,
        model_name: String,
        model_id: uuid::Uuid,
        provider_id: uuid::Uuid,
    ) -> Self {
        Self {
            registry,
            ctx: Mutex::new(ctx),
            send_request,
            tx,
            provider_type,
            model_name,
            model_id,
            provider_id,
        }
    }

    /// Seed the per-iteration context metadata exactly as the legacy loop does
    /// (`streaming.rs`): provider/model identity + memoized tool-capability +
    /// resolved available files — so title/file/other hooks behave identically.
    async fn seed_metadata(&self, ctx: &mut StreamContext) {
        let m = &mut ctx.metadata;
        m.insert("provider_type".into(), serde_json::json!(self.provider_type));
        m.insert("model_name".into(), serde_json::json!(self.model_name));
        m.insert("model_id".into(), serde_json::json!(self.model_id.to_string()));
        m.insert(
            "provider_id".into(),
            serde_json::json!(self.provider_id.to_string()),
        );
        let tool_capable =
            crate::modules::file::available_files::ensure_model_tools_capable(m).await;
        if tool_capable {
            crate::modules::file::available_files::seed_available_files(
                m,
                ctx.conversation_id,
                ctx.user_id,
            )
            .await;
        }
    }
}

#[async_trait]
impl AgentExtension for RegistryBridge {
    fn name(&self) -> &str {
        "chat_registry_bridge"
    }

    fn order(&self) -> i32 {
        // Runs after the crate's assemble (system/tools from the empty contribute
        // phase) so the real chat context + tools land on the request.
        1000
    }

    async fn before_model(&self, req: &mut ChatRequest) -> Result<Flow, AppError> {
        let mut ctx = self.ctx.lock().await;
        // Match the legacy loop's 1-indexed iteration bump before each LLM call.
        ctx.iteration += 1;
        // Seed provider/model/tool-capability/files metadata (legacy parity) so the
        // extensions' before_llm_call + the later after_llm_call read the same ctx.
        self.seed_metadata(&mut ctx).await;
        let action = self
            .registry
            .call_before_llm_call(&mut ctx, req, &self.send_request, self.tx.as_ref())
            .await?;
        Ok(match action {
            BeforeLlmAction::Continue => Flow::Continue,
            // Complete / CompleteWithContent short-circuit the LLM call. The crate
            // ends the turn on ShortCircuit; a `CompleteWithContent{text}` early
            // answer is a rare pre-LLM optimization — its text is handled by the
            // dispatcher's short-circuit fallback (persist + emit) when needed.
            _ => Flow::ShortCircuit,
        })
    }

    /// Run the registry's `after_llm_call` SIDE-EFFECTS (title generation, memory
    /// extraction) — but ONLY on the FINAL round (the assistant message requested
    /// no tools). On a tool round the ports own execution+continuation, and the MCP
    /// extension's `after_llm_call` must NOT run (it would re-execute tools); on a
    /// no-tool message it early-returns `Complete`, so only the pure side-effect
    /// extensions do work. The returned `ExtensionAction` is ignored — continuation
    /// is the crate loop's native decision.
    async fn after_round(&self, msg: &ChatMessage) -> Result<Flow, AppError> {
        let has_tool_use = msg
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
        if has_tool_use {
            return Ok(Flow::Continue);
        }
        let ctx = self.ctx.lock().await;
        let assistant_message_id = match ctx.message_id {
            Some(id) => id,
            None => return Ok(Flow::Continue),
        };
        // Fetch the just-persisted assistant message (the `after_llm_call` contract).
        if let Some(final_message) = crate::core::Repos
            .chat
            .core
            .get_message(assistant_message_id)
            .await?
        {
            let _ = self
                .registry
                .call_after_llm_call(&ctx, &final_message, self.tx.as_ref())
                .await?;
        }
        Ok(Flow::Continue)
    }
}
