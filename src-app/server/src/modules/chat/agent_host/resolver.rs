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

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use agent_core::{IdempotencyKey, ModelResolver, ToolCall, ToolProvider, ToolResult, ToolScope};
use ai_providers::{ContentBlock, Provider, Tool};
use async_trait::async_trait;
use axum::response::sse::Event;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::workflow::dispatch::{
    call_mcp_tool, resolve_tool_server, CancelSignal, McpCallScope,
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
    // Audience-only bypass (parity with the MCP extension's `execute_tool`): if any
    // content block is annotated `audience: ["user"]` EXACTLY, the tool's output is
    // meant for the user, not the model — the turn ends with it (no continuation).
    let terminal = r.content.iter().any(|c| {
        c.content
            .get("annotations")
            .and_then(|a| a.get("audience"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.len() == 1 && arr[0].as_str() == Some("user"))
            .unwrap_or(false)
    });
    ToolResult {
        content: vec![ContentBlock::Text { text }],
        is_error: r.is_error,
        structured_content: r.structured_content,
        terminal,
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
    /// SSE sink for the `mcpToolStart` / `mcpToolComplete` lifecycle frames the
    /// legacy `execute_tool` emitted — the chat UI renders tool activity from these.
    tx: Option<UnboundedSender<Result<Event, Infallible>>>,
    /// Chat-turn identity threaded into the shared `call_mcp_tool` chokepoint so it
    /// routes through the SAME sampling/journaling machinery the legacy path uses
    /// (a sampling server gets a `new_with_sampling` session; the journal row gets
    /// the real branch/message/tool_use context).
    branch_id: Uuid,
    message_id: Uuid,
    model_id: Uuid,
}

impl ChatToolProvider {
    /// `token` is a clone of the per-`assistant_message_id` stop-generation token
    /// (`CANCELLATION_TRACKER.create_token(...)`), so a stop request aborts an
    /// in-flight tool call for this turn. `tx` is the extension-event SSE sender
    /// (for tool-lifecycle frames).
    pub fn new(
        user_id: Uuid,
        conversation_id: Option<Uuid>,
        token: CancellationToken,
        tx: Option<UnboundedSender<Result<Event, Infallible>>>,
        branch_id: Uuid,
        message_id: Uuid,
        model_id: Uuid,
    ) -> Self {
        Self {
            user_id,
            conversation_id,
            cancel: ChatCancel::new(token),
            tx,
            branch_id,
            message_id,
            model_id,
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

    // NOTE: `_idem` (the crate's per-call idempotency key) is intentionally NOT
    // forwarded — `call_mcp_tool` only persists an idempotency key for `Workflow`
    // source (chat rows key their journal by the real LLM `tool_use_id`, and chat
    // has no durable workflow-run resume). Passing `Some(idem)` here was a silent
    // no-op; we pass `None` below to make that explicit. Named `_idem` (not a
    // blanket `#[allow(unused_variables)]`) so a genuinely-dead local still warns.
    async fn call(
        &self,
        run_id: Uuid,
        call: ToolCall,
        _idem: IdempotencyKey,
    ) -> Result<ToolResult, AppError> {
        let (server_name, tool_name) = split_tool_name(&call.name);
        let scope = McpCallScope {
            user_id: self.user_id,
            conversation_id: self.conversation_id,
            run_id,
        };
        // Tool-lifecycle SSE (parity with the legacy `execute_tool`): start before,
        // complete after — so the chat UI shows the tool running + its result.
        use crate::modules::mcp::chat_extension::helpers::{
            send_tool_complete_event, send_tool_start_event,
        };
        send_tool_start_event(self.tx.as_ref(), &call.id, &tool_name, &server_name, &call.input)
            .await;
        // DEC-17: the chat host passes `enforce_conversation_disabled = false` —
        // chat applies disabled-server filtering at attach time, not call time.
        let outcome = call_mcp_tool(
            &scope,
            &server_name,
            &tool_name,
            call.input.clone(),
            false,
            &self.cancel,
            // Route through the shared chokepoint WITH the chat context so a
            // sampling server gets a `new_with_sampling` session + the journal row
            // carries branch/message/tool_use (parity with the legacy path).
            Some(crate::modules::workflow::dispatch::ChatCallCtx {
                branch_id: self.branch_id,
                message_id: self.message_id,
                tool_use_id: call.id.clone(),
                model_id: self.model_id,
            }),
            None,
            None, // idempotency_key: chat has no workflow-run resume (see call() note)
            crate::modules::mcp::tool_calls::models::McpToolCallSource::Chat,
        )
        .await;
        match outcome {
            Ok((_server_id, result)) => {
                let mut agent = mcp_to_agent_result(result);
                // Memory `remember`/`forget` are built-in side-effect self-saves —
                // their result isn't something the model must reason about, so when
                // only these ran the turn finalizes without a no-op continuation
                // (parity with mcp.rs `is_side_effect_tool` / Track B).
                if let Ok(sid) = Uuid::parse_str(&server_name) {
                    if sid == crate::modules::memory_mcp::memory_mcp_server_id()
                        && matches!(tool_name.as_str(), "remember" | "forget")
                    {
                        agent.terminal = true;
                    }
                }
                let text = agent.content.iter().find_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                });
                send_tool_complete_event(
                    self.tx.as_ref(),
                    &call.id,
                    &tool_name,
                    &server_name,
                    agent.is_error,
                    text,
                )
                .await;
                Ok(agent)
            }
            Err(McpToolCallError::Cancelled) => {
                send_tool_complete_event(
                    self.tx.as_ref(),
                    &call.id,
                    &tool_name,
                    &server_name,
                    true,
                    Some("cancelled"),
                )
                .await;
                Err(AppError::internal_error("chat: tool call cancelled"))
            }
            Err(McpToolCallError::Failed(m)) => {
                send_tool_complete_event(
                    self.tx.as_ref(),
                    &call.id,
                    &tool_name,
                    &server_name,
                    true,
                    Some(&m),
                )
                .await;
                Err(AppError::internal_error(m))
            }
        }
    }

    fn is_trusted(&self, server: &str) -> bool {
        // The loop passes `call.server.unwrap_or(call.name)`; the crate sets
        // `server = None`, so `server` is the namespaced tool name. Chat's MCP
        // extension namespaces as `<server_id>__<tool>` — parse the prefix as the
        // server-id uuid and trust built-in servers (parity with mcp.rs's
        // `is_builtin_server_id`, which drives the approval bypass). Fall back to a
        // NAME lookup for the workflow-style `<server_name>__<tool>` scheme.
        let (prefix, _) = split_tool_name(server);
        match uuid::Uuid::parse_str(&prefix) {
            Ok(server_id) => {
                crate::modules::mcp::chat_extension::mcp::is_builtin_server_id(server_id)
            }
            // A NAME-prefixed tool is NOT auto-trusted: `builtin_server_id_by_name`
            // includes `code_sandbox`/`control_mcp` (which `is_builtin_server_id`
            // deliberately EXCLUDES from the approval bypass), so trusting a name
            // prefix would auto-approve sandbox code execution. Chat namespaces by
            // server-id uuid, so this branch is not hit today; keep it fail-safe.
            Err(_) => false,
        }
    }
}
