//! Chat agent-host ports: the model resolver + the tool provider (re-home wave 5).
//!
//! Two of the six `agent_core` seams, in their CHAT flavor. They mirror the
//! workflow twins in `modules::workflow::agent_dispatch`
//! ([`WorkflowModelResolver`] / `McpToolProvider`) closely — the ONLY behavioral
//! divergence is the disabled-server gate: the chat host passes
//! `enforce_conversation_disabled = false` to the shared `call_mcp_tool`, exactly
//! preserving chat's current non-enforcement (DEC-17).
//!
//! # UX walk
//! A user sends a chat message. The model, mid-turn, asks to run a tool. The core
//! loop calls [`ChatToolProvider::list`] to learn which tools exist — it gathers
//! them from the conversation's attached servers (the built-in server NAMES the
//! ported context-injector extensions pushed onto `TurnContext.tool_scope.servers`),
//! resolving + listing each via the same MCP session path chat uses today, and
//! namespaces each as `"<server>__<tool>"`. The model picks one; the loop calls
//! [`ChatToolProvider::call`], which routes through the shared `call_mcp_tool`
//! chokepoint, executes against the live MCP session, and hands the result back to
//! the model — so the tool surface is byte-for-byte the same set the user sees in
//! chat today, executed the same way. [`ChatModelResolver`] is consulted only when
//! the loop needs a per-child / reviewer provider (fan-out / review), resolving a
//! `model_id` to a `Provider` under the user's RBAC.
//!
//! # Infra-integration walk (what this touches; behavior to preserve)
//! - **MCP session manager + recording chokepoint** — both `list` and `call` open
//!   sessions via `crate::modules::mcp::client::manager::global()`. The
//!   `mcp_tool_calls` journal row is written ONCE inside `McpSession::call_tool`
//!   (the shared chokepoint); we must NOT double-record here. Sessions are stamped
//!   with `McpToolCallSource::Chat` so the recorded row's `source` reads `chat`
//!   (distinct from the workflow twin's `Workflow`).
//! - **Disabled-server gate** — SKIPPED here (`enforce_conversation_disabled =
//!   false`). Chat applies its per-conversation disabled-server filtering earlier,
//!   at attach time (which servers land in `tool_scope.servers`), so re-enforcing
//!   at call time would be a behavior change. Preserve `false`.
//! - **Cancellation** — the chat stop-generation token (per-`assistant_message_id`,
//!   from `CANCELLATION_TRACKER`) is wrapped as a [`CancelSignal`] and raced against
//!   each MCP call inside `call_mcp_tool`'s `tokio::select!`. A stop-generation
//!   request therefore aborts an in-flight tool call, mapping to a cancelled turn.
//! - **RBAC on model resolve** — [`ChatModelResolver`] DENIES an inaccessible /
//!   disabled model (not-found / forbidden / bad-request), the boundary the crate
//!   never crosses on its own. This mirrors `WorkflowModelResolver` exactly.

use std::sync::Arc;
use std::time::Duration;

use agent_core::{IdempotencyKey, ModelResolver, ToolCall, ToolProvider, ToolResult, ToolScope};
use ai_providers::{ContentBlock, Provider, Tool};
use async_trait::async_trait;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::workflow::dispatch::{
    builtin_server_id_by_name, call_mcp_tool, resolve_tool_server, CancelSignal, McpCallScope,
    McpToolCallError,
};
use crate::utils::cancellation::CancellationToken;

// ============================================================
// Model resolver (chat flavor — mirrors WorkflowModelResolver, DEC-16/B)
// ============================================================

/// Resolves a `model_id` → a `Provider` under the acting user's RBAC. Used by the
/// core loop's fan-out / reviewer paths to mint a per-child / reviewer provider
/// without the crate touching the DB. DENIES an inaccessible or disabled model.
pub struct ChatModelResolver;

#[async_trait]
impl ModelResolver for ChatModelResolver {
    async fn resolve(&self, model_id: Uuid, user_id: Uuid) -> Result<Arc<Provider>, AppError> {
        use crate::core::Repos;
        let model = Repos
            .llm_model
            .get_by_id(model_id)
            .await?
            .ok_or_else(|| AppError::not_found("Model"))?;
        if !model.enabled {
            return Err(AppError::bad_request(
                "MODEL_DISABLED",
                "this model is currently disabled and cannot be used",
            ));
        }
        let has_access = Repos
            .user_group_llm_provider
            .user_has_access_to_provider(user_id, model.provider_id)
            .await
            .map_err(AppError::from)?;
        if !has_access {
            return Err(AppError::forbidden(
                "ACCESS_DENIED",
                "you do not have access to this model",
            ));
        }
        let (provider, ..) =
            crate::modules::chat::core::ai_provider::create_provider_from_model_id(model_id, user_id)
                .await?;
        Ok(provider)
    }
}

// ============================================================
// Cancellation bridge (chat stop-generation → CancelSignal)
// ============================================================

/// Adapts the chat stop-generation [`CancellationToken`] (per-`assistant_message_id`,
/// polled from `CANCELLATION_TRACKER`) to the workflow `CancelSignal` seam that
/// `call_mcp_tool` awaits. The token exposes only a poll (`is_cancelled()`, no
/// notify), so `cancelled()` polls it on a short cadence — the future is only ever
/// raced against a live tool call inside a `tokio::select!`, so it is simply
/// dropped when the call finishes first (no busy-loop after the turn ends). The
/// token latches once cancelled, so repeated polls stay `true`.
pub struct ChatCancel {
    token: CancellationToken,
}

impl ChatCancel {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }
}

#[async_trait]
impl CancelSignal for ChatCancel {
    async fn cancelled(&self) {
        loop {
            if self.token.is_cancelled().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

// ============================================================
// Tool provider (chat flavor — mirrors McpToolProvider, DEC-17)
// ============================================================

/// Split a namespaced tool name `"<server>__<tool>"` (as emitted by
/// [`ChatToolProvider::list`]) back into `(server_name, tool_name)`. Splits on the
/// FIRST `__` (mirrors the chat/workflow `server_id__name` scheme).
fn split_tool_name(name: &str) -> (String, String) {
    match name.find("__") {
        Some(idx) => (name[..idx].to_string(), name[idx + 2..].to_string()),
        None => (String::new(), name.to_string()),
    }
}

/// Flatten an MCP `ToolResult` into an `agent_core::ToolResult`: concatenate its
/// text blocks into one `Text` block (mirrors the workflow twin's
/// `mcp_to_agent_result`), preserving `is_error` + `structured_content`.
fn mcp_to_agent_result(r: crate::modules::mcp::client::traits::ToolResult) -> ToolResult {
    let mut text = String::new();
    for c in &r.content {
        if c.content.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(t) = c.content.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
    }
    // No text blocks (e.g. an image-only or structured-only result) → stringify
    // the raw content so the model still sees something actionable.
    if text.is_empty() && !r.content.is_empty() {
        text = serde_json::to_string(&r.content).unwrap_or_default();
    }
    ToolResult {
        content: vec![ContentBlock::Text { text }],
        is_error: r.is_error,
        structured_content: r.structured_content,
    }
}

/// The chat agent's tool surface — the conversation's attached servers (server
/// NAMES on `ToolScope.servers`), resolved to MCP tools and routed through the
/// shared `call_mcp_tool` path with the conversation disabled-server gate OFF
/// (chat's current behavior — DEC-17).
pub struct ChatToolProvider {
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    /// The chat stop-generation token, wrapped so `call_mcp_tool` can await it.
    cancel: ChatCancel,
}

impl ChatToolProvider {
    /// `token` is a clone of the per-`assistant_message_id` stop-generation token
    /// (`CANCELLATION_TRACKER.create_token(...)`), so a stop request aborts an
    /// in-flight tool call for this turn.
    pub fn new(user_id: Uuid, conversation_id: Option<Uuid>, token: CancellationToken) -> Self {
        Self {
            user_id,
            conversation_id,
            cancel: ChatCancel::new(token),
        }
    }
}

#[async_trait]
impl ToolProvider for ChatToolProvider {
    async fn list(&self, scope: &ToolScope) -> Result<Vec<Tool>, AppError> {
        let manager = crate::modules::mcp::client::manager::global()
            .ok_or_else(|| AppError::internal_error("MCP session manager not initialized"))?;
        let mut tools = Vec::new();
        for server_name in &scope.servers {
            // A server the user can't reach (or that fails to list) contributes no
            // tools rather than failing the whole turn.
            let server_id = match resolve_tool_server(self.user_id, server_name).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("chat agent: server '{server_name}' not accessible: {e}");
                    continue;
                }
            };
            let session = match manager
                .get_or_create_with_context(
                    server_id,
                    self.user_id,
                    self.conversation_id,
                    None,
                    None,
                    None,
                    crate::modules::mcp::tool_calls::models::McpToolCallSource::Chat,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("chat agent: open session for '{server_name}': {e}");
                    continue;
                }
            };
            let listed = {
                let mut guard = session.write().await;
                guard.list_tools().await
            };
            match listed {
                Ok(list) => {
                    for t in list {
                        // Namespace the tool name by server NAME so `call` /
                        // `is_trusted` can route back (the crate sets
                        // `ToolCall.server = None`).
                        let name = format!("{server_name}__{}", t.name);
                        tools.push(Tool::function(
                            name,
                            t.description.unwrap_or_default(),
                            t.input_schema,
                        ));
                    }
                }
                Err(e) => tracing::warn!("chat agent: list tools for '{server_name}': {e}"),
            }
        }
        Ok(tools)
    }

    async fn call(
        &self,
        run_id: Uuid,
        call: ToolCall,
        idem: IdempotencyKey,
    ) -> Result<ToolResult, AppError> {
        let (server_name, tool_name) = split_tool_name(&call.name);
        let scope = McpCallScope {
            user_id: self.user_id,
            conversation_id: self.conversation_id,
            run_id,
        };
        // DEC-17: the chat host passes `enforce_conversation_disabled = false` —
        // chat applies disabled-server filtering at attach time, not call time.
        match call_mcp_tool(
            &scope,
            &server_name,
            &tool_name,
            call.input,
            false,
            &self.cancel,
            None,
            Some(idem),
            crate::modules::mcp::tool_calls::models::McpToolCallSource::Chat,
        )
        .await
        {
            Ok((_server_id, result)) => Ok(mcp_to_agent_result(result)),
            Err(McpToolCallError::Cancelled) => {
                Err(AppError::internal_error("chat: tool call cancelled"))
            }
            Err(McpToolCallError::Failed(m)) => Err(AppError::internal_error(m)),
        }
    }

    fn is_trusted(&self, server: &str) -> bool {
        // The loop passes `call.server.unwrap_or(call.name)`; since the crate sets
        // `server = None`, `server` is the namespaced tool name — parse its prefix
        // and treat any built-in server as trusted (mirrors the workflow twin).
        let (server_name, _) = split_tool_name(server);
        builtin_server_id_by_name(&server_name).is_some()
    }
}
