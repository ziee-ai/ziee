//! Shared MCP tool-call chokepoint (moved here from `workflow::dispatch` so it is
//! SHARED INFRA both the workflow dispatcher AND the chat agent host import from
//! `mcp/` — neither feature module imports the other for it; §9 DAG rule).
//!
//! Owns: the built-in NAME->id map, `resolve_tool_server`, `McpCallScope` (the
//! minimal run identity extracted from `RunContext`), `McpToolCallError`, the
//! `CancelSignal` seam (its one workflow impl — `RunHandle` — stays in
//! `workflow::dispatch`, which owns that type), `ChatCallCtx`, the `call_mcp_tool`
//! path, and the two agent-core adapters (`split_tool_name` / `mcp_to_agent_result`)
//! previously copy-pasted in both hosts.

use agent_core::ToolResult;
use ai_providers::ContentBlock;
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::common::AppError;

/// Resolve a `tool` step's `server` NAME to a server id the running user may
/// call. Built-ins resolve by stable name (their OWN permission still gates the
/// call); user/system servers resolve within the user's accessible enabled set.
/// Pure NAME → built-in server-id mapping used by `resolve_tool_server`.
/// Extracted so the workflow↔built-in wiring (incl. the workflow→memory MCP
/// seam) is unit-testable without a DB. `None` for non-built-in names.
pub(crate) fn builtin_server_id_by_name(server_name: &str) -> Option<Uuid> {
    match server_name {
        "web_search" => Some(crate::modules::web_search::web_search_server_id()),
        "bio" => Some(crate::modules::bio_mcp::bio_mcp_server_id()),
        "lit_search" => Some(crate::modules::lit_search::lit_search_server_id()),
        "citations" => Some(crate::modules::citations::citations_server_id()),
        "memory" => Some(crate::modules::memory_mcp::memory_mcp_server_id()),
        "files" => Some(crate::modules::files_mcp::files_mcp_server_id()),
        "code_sandbox" => Some(crate::modules::code_sandbox::code_sandbox_server_id()),
        _ => None,
    }
}

pub(crate) async fn resolve_tool_server(
    user_id: Uuid,
    server_name: &str,
) -> Result<Uuid, AppError> {
    use crate::core::Repos;
    let builtin = builtin_server_id_by_name(server_name);
    if let Some(id) = builtin {
        // Built-in: allowed iff registered + enabled. The server's own
        // permission (bio::query / web_search::use / ...) gates the call.
        match Repos.mcp.get_any_server(id).await? {
            Some(s) if s.enabled => return Ok(id),
            _ => {
                return Err(AppError::forbidden(
                    "WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE",
                    format!("built-in server '{server_name}' is not enabled"),
                ));
            }
        }
    }
    // User-owned / group-assigned system server, by name (enabled + accessible).
    let servers = crate::modules::mcp::chat_extension::helpers::get_all_accessible_config(
        Repos.pool(),
        user_id,
    )
    .await?;
    if let Some(s) = servers
        .into_iter()
        .find(|s| s.name == server_name && s.enabled)
    {
        return Ok(s.id);
    }
    Err(AppError::forbidden(
        "WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE",
        format!("server '{server_name}' is not accessible to this user"),
    ))
}

/// The minimal run identity `call_mcp_tool` needs — extracted from `RunContext`
/// so the shared MCP call path is reachable from BOTH the `ToolDispatcher` (which
/// has a full `RunContext`) and the agent host's `McpToolProvider` (which holds
/// only these three fields).
#[derive(Debug, Clone, Copy)]
pub(crate) struct McpCallScope {
    pub user_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub run_id: Uuid,
}

// ============================================================
// ITEM-50: full-disclosure approval prompt (data-egress review)
// ============================================================
//
// An external-tool approval is a DATA-EGRESS decision: the human must be able to
// see the CONCRETE destination host their data would be sent to and the tool's
// FULL, EXACT advertised description (poisoning hides in a truncated/summarized
// description). These two helpers surface both from the ALREADY-resolved server
// row (no new SSRF/host policy — just the display host) + a best-effort live
// `tools/list`. Both agent hosts (chat `ChatHumanGate` + the legacy `mcp.rs`
// approval-classification path) call them at the point they build the
// `McpApprovalRequired` SSE frame.

/// Resolve the external destination HOST to name on a data-egress approval card.
/// Pure + unit-testable.
///
/// Returns just the HOST (never the full URL — no path/query/credentials leak).
/// `None` for a built-in / loopback / stdio server: those have no meaningful
/// EXTERNAL destination (built-ins are in-process loopback; stdio is a local
/// subprocess). For an `is_system` server the public `url` is redacted in list
/// views, but this operates on the INTERNAL row (from `get_any_server`), so the
/// real host is resolved SERVER-SIDE without depending on the redacted view.
pub(crate) fn resolve_dest_host(
    server: &crate::modules::mcp::models::McpServer,
) -> Option<String> {
    use crate::modules::mcp::models::TransportType;
    // Built-in servers are in-process loopback — no external egress destination.
    if server.is_built_in {
        return None;
    }
    match server.transport_type {
        TransportType::Http | TransportType::Sse => {
            let raw = server.url.as_deref()?;
            let host = url::Url::parse(raw).ok()?.host_str()?.to_string();
            if is_local_egress_host(&host) {
                // Loopback / local — a user-registered server pointed at
                // localhost is not a meaningful EXTERNAL destination.
                return None;
            }
            Some(host)
        }
        // stdio: a local subprocess, no network destination.
        TransportType::Stdio => None,
    }
}

/// Loopback / local hostnames that are not a meaningful EXTERNAL egress target.
fn is_local_egress_host(host: &str) -> bool {
    matches!(
        host,
        "localhost" | "127.0.0.1" | "::1" | "[::1]" | "0.0.0.0"
    ) || host.ends_with(".localhost")
}

/// Best-effort resolve the tool's EXACT advertised description for an approval
/// card. Uses the process-wide session manager to `tools/list` the (pooled)
/// server session and returns the matching tool's description verbatim (NEVER
/// truncated/summarized). Any failure (no manager installed, unreachable server,
/// tool absent) yields `None` so it NEVER blocks the approval SSE frame — the
/// card simply falls back to a generic line.
pub(crate) async fn resolve_tool_description(
    server_id: Option<Uuid>,
    user_id: Uuid,
    tool_name: &str,
) -> Option<String> {
    use crate::modules::mcp::tool_calls::models::McpToolCallSource;
    let server_id = server_id?;
    let manager = crate::modules::mcp::client::manager::global()?;
    let session = manager
        .get_or_create_with_context(
            server_id,
            user_id,
            None,
            None,
            None,
            None,
            McpToolCallSource::Rest,
        )
        .await
        .ok()?;
    let tools = {
        let mut session = session.write().await;
        session.list_tools().await.ok()?
    };
    tools
        .into_iter()
        .find(|t| t.name == tool_name)
        .and_then(|t| t.description)
        .filter(|d| !d.is_empty())
}

/// Outcome of the shared MCP call path (DEC-17).
pub(crate) enum McpToolCallError {
    /// The run's cancel handle fired mid-call.
    Cancelled,
    /// Any other failure (server inaccessible, disabled, session/RPC error).
    Failed(String),
}

/// The shared MCP tool-call path (ITEM-21 / DEC-17) — resolve the server NAME,
/// apply the conversation/default disabled-server gate (only when
/// `enforce_conversation_disabled`), open a recording session linked to the run,
/// and invoke the tool with the given (already-rendered) `args`. Returns the
/// resolved `server_id` (the caller needs it for `resource_link` persistence) +
/// the raw MCP `ToolResult`.
///
/// `ToolDispatcher` calls this with `enforce_conversation_disabled = true`
/// (behaviour-preserving — same gate it applied inline before the extraction).
/// The workflow agent host's `McpToolProvider` also passes `true`; the chat host
/// (a later stage) passes `false` to preserve its current non-enforcement.
/// A cancellation signal `call_mcp_tool` can await, decoupled from the workflow
/// `RunHandle` so a non-workflow host (chat's stop-generation) can supply its own
/// token without owning a `RunHandle`. `RunHandle` implements it directly, so the
/// workflow call sites are unchanged in behavior.
#[async_trait]
pub(crate) trait CancelSignal: Send + Sync {
    /// Resolves when cancellation is requested (or immediately if already cancelled).
    async fn cancelled(&self);
}

/// Chat-turn context threaded into the shared MCP call chokepoint by the
/// agent-core CHAT host so it routes through the SAME machinery the legacy path
/// uses: (a) a sampling server gets an ephemeral `new_with_sampling` session (so
/// server→host sampling round-trips fire), and (b) the recorded `mcp_tool_calls`
/// row carries the real `branch_id`/`message_id`/`tool_use_id` context (journaling
/// parity). Workflow callers pass `None` → the pooled, chat-context-free path is
/// byte-identical to before.
pub(crate) struct ChatCallCtx {
    pub branch_id: Uuid,
    pub message_id: Uuid,
    pub tool_use_id: String,
    pub model_id: Uuid,
}

pub(crate) async fn call_mcp_tool(
    scope: &McpCallScope,
    server_name: &str,
    tool_name: &str,
    args: Value,
    enforce_conversation_disabled: bool,
    cancel: &dyn CancelSignal,
    // Chat-only context (sampling session + journal linkage); `None` for workflow.
    chat_ctx: Option<ChatCallCtx>,
    // ITEM-12: reviewer risk classification (`low`/`high`/`critical`) stamped
    // onto the recorded `mcp_tool_calls` row (`None` for the plain tool step).
    review_classification: Option<String>,
    // ITEM-16: stable per-call idempotency key `<run_id>:<turn>:<ordinal>` threaded
    // into the MCP call context so an in-flight side-effecting call is identifiable
    // on resume (`None` for the plain tool step).
    idempotency_key: Option<String>,
    // Which surface the recorded `mcp_tool_calls` row attributes this call to
    // (`Workflow` for the workflow tool step + agent host; `Chat` for the chat
    // agent host, so its rows read `chat` like today's chat tool calls).
    source: crate::modules::mcp::tool_calls::models::McpToolCallSource,
) -> Result<(Uuid, crate::modules::mcp::client::traits::ToolResult), McpToolCallError> {
    // The namespaced prefix is either a server NAME (workflow's `McpToolProvider`
    // list) or a server ID uuid (chat's MCP extension namespaces tools as
    // `<server_id>__<tool>`). The two hosts use DISJOINT schemes: the chat host
    // (`chat_ctx` present) always sends a server-id uuid; the workflow host always
    // sends a server NAME. Take the raw-id path ONLY for the chat host, so a
    // workflow server that happens to be NAMED a literal uuid is resolved by name
    // (not misread as an id).
    let server_id = match uuid::Uuid::parse_str(server_name) {
        Ok(id) if chat_ctx.is_some() => {
            // SECURITY: re-validate the (model-supplied) server id against the acting
            // user's accessible set — a built-in (enabled) OR a server the user's
            // groups are assigned to. Without this the raw-uuid path would let a
            // prompt-injected tool name reach an arbitrary server_id the user isn't
            // assigned to, executing with that server's admin-configured secret
            // headers (cross-group authz bypass). Mirrors the legacy accessible-set
            // check at `mcp.rs` (execute_approved_tools_sync).
            let accessible = match crate::core::Repos.mcp.get_any_server(id).await {
                // A BUILT-IN server (control/skill/workflow/memory/… — `is_built_in`,
                // some also `is_system`) is accessible to any user when enabled; its
                // per-tool authz is enforced downstream at the JSON-RPC handler
                // (`control::use`, etc.). Some built-ins (control) are NOT in the
                // approval-bypass `is_builtin_server_id` set AND are `is_system` (so
                // redacted out of `get_all_accessible_config`), which previously made
                // them unreachable on the agent-core chat path — the legacy
                // `execute_tool` path treats built-ins as accessible, so match it.
                Ok(Some(s))
                    if s.enabled
                        && (s.is_built_in
                            || crate::modules::mcp::chat_extension::mcp::is_builtin_server_id(id)) =>
                {
                    true
                }
                // External / user-registered server: require it in the user's
                // accessible set (unchanged — the cross-group-authz-bypass guard).
                _ => crate::modules::mcp::chat_extension::helpers::get_all_accessible_config(
                    crate::core::Repos.pool(),
                    scope.user_id,
                )
                .await
                .map(|servers| servers.iter().any(|s| s.id == id && s.enabled))
                .unwrap_or(false),
            };
            if !accessible {
                return Err(McpToolCallError::Failed(format!(
                    "server '{id}' is not accessible to this user"
                )));
            }
            id
        }
        // Workflow host (name scheme), OR a non-uuid server_name: resolve by name.
        _ => match resolve_tool_server(scope.user_id, server_name).await {
            Ok(id) => id,
            Err(e) => return Err(McpToolCallError::Failed(e.to_string())),
        },
    };

    // E8: reject a server OR a specific tool the user disabled. Conversation-
    // scoped when a conversation is present; otherwise the user's DEFAULT MCP
    // disabled set (the scheduled/standalone case). Fail CLOSED on any DB error
    // (security gate). Skipped entirely when `enforce_conversation_disabled` is
    // false (the chat host's current behavior — DEC-17).
    if enforce_conversation_disabled {
        if let Some(conv_id) = scope.conversation_id {
            let settings = match crate::core::repository::Repos
                .mcp_settings
                .get(crate::modules::mcp::settings::models::McpScope::Conversation(conv_id))
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    return Err(McpToolCallError::Failed(format!(
                        "tool: could not resolve server policy: {e}"
                    )));
                }
            };
            if let Some(settings) = settings {
                let disabled: Vec<
                    crate::modules::mcp::chat_extension::approval::models::DisabledServer,
                > = serde_json::from_value(settings.disabled_servers).unwrap_or_default();
                if disabled.iter().any(|d| {
                    d.server_id == server_id
                        && (d.is_server_disabled() || d.is_tool_disabled(tool_name))
                }) {
                    return Err(McpToolCallError::Failed(format!(
                        "tool '{tool_name}' on server '{server_name}' is disabled in this conversation"
                    )));
                }
            }
        } else {
            let defaults = match crate::modules::mcp::chat_extension::defaults::repository::get_user_defaults(
                crate::core::Repos.pool(),
                scope.user_id,
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    return Err(McpToolCallError::Failed(format!(
                        "tool: could not resolve user MCP defaults: {e}"
                    )));
                }
            };
            if let Some(defaults) = defaults {
                let disabled = defaults.get_disabled_servers();
                if disabled.iter().any(|d| {
                    d.server_id == server_id
                        && (d.is_server_disabled() || d.is_tool_disabled(tool_name))
                }) {
                    return Err(McpToolCallError::Failed(format!(
                        "tool '{tool_name}' on server '{server_name}' is disabled in your default MCP settings"
                    )));
                }
            }
        }
    }

    let manager = match crate::modules::mcp::client::manager::global() {
        Some(m) => m,
        None => {
            return Err(McpToolCallError::Failed(
                "MCP session manager not initialized".into(),
            ));
        }
    };
    // Session selection. The CHAT host (chat_ctx = Some) routes through the SAME
    // shared machinery the legacy path uses: a `supports_sampling` server gets an
    // ephemeral `new_with_sampling` session (server→host sampling round-trips), and
    // the pooled path carries the real branch/message/tool_use context (journaling
    // parity). Workflow (chat_ctx = None) is unchanged — pooled, no chat context.
    let session: std::sync::Arc<tokio::sync::RwLock<crate::modules::mcp::client::session::McpSession>> =
        if let Some(cc) = &chat_ctx {
            // Lightweight lookup (works for built-in loopback servers too, unlike
            // `resolve_server_for_session` which is for external un-redacted URLs)
            // to decide the sampling-vs-pooled branch WITHOUT an early-return that
            // would skip `call_tool` (and thus the journal recording) for built-ins.
            let supports_sampling = crate::core::Repos
                .mcp
                .get_any_server(server_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.supports_sampling)
                .unwrap_or(false);
            if supports_sampling {
                let server_row = match manager.resolve_server_for_session(server_id).await {
                    Ok(s) => s,
                    Err(e) => return Err(McpToolCallError::Failed(format!("tool: resolve server: {e}"))),
                };
                // Sampling server → fresh ephemeral session WITH the host-LLM
                // handler (parity with legacy `execute_tool`). No pooled fallback:
                // a pooled sampling session deadlocks the SSE round-trip.
                let handler: std::sync::Arc<dyn crate::modules::mcp::sampling::handler::SamplingHandler> =
                    match crate::modules::mcp::sampling::handler::ChatSamplingHandler::new(cc.model_id, scope.user_id).await {
                        Ok(h) => std::sync::Arc::new(h),
                        Err(e) => return Err(McpToolCallError::Failed(format!("tool: sampling handler init: {e}"))),
                    };
                let mut s = match crate::modules::mcp::client::session::McpSession::new_with_sampling(server_row.clone(), handler).await {
                    Ok(s) => s,
                    Err(e) => return Err(McpToolCallError::Failed(format!("tool: sampling session: {e}"))),
                };
                s.set_call_context(crate::modules::mcp::tool_calls::models::McpCallContext {
                    user_id: Some(scope.user_id),
                    conversation_id: scope.conversation_id,
                    branch_id: Some(cc.branch_id),
                    message_id: Some(cc.message_id),
                    tool_use_id: Some(cc.tool_use_id.clone()),
                    source,
                    server_name: server_row.name.clone(),
                    is_built_in: server_row.is_built_in,
                    ..Default::default()
                });
                std::sync::Arc::new(tokio::sync::RwLock::new(s))
            } else {
                match manager
                    .get_or_create_with_context(
                        server_id,
                        scope.user_id,
                        scope.conversation_id,
                        Some(cc.branch_id),
                        Some(cc.message_id),
                        Some(cc.tool_use_id.clone()),
                        source,
                    )
                    .await
                {
                    Ok(s) => s,
                    Err(e) => return Err(McpToolCallError::Failed(format!("tool: open session: {e}"))),
                }
            }
        } else {
            match manager
                .get_or_create_with_context(
                    server_id,
                    scope.user_id,
                    scope.conversation_id,
                    None, // branch_id
                    None, // message_id — a workflow run has no chat message
                    None, // tool_use_id — not an LLM ContentBlock::ToolUse
                    source,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => return Err(McpToolCallError::Failed(format!("tool: open session: {e}"))),
            }
        };

    let call = async {
        let mut guard = session.write().await;
        // E4: link the recorded `mcp_tool_calls` row to this run — WORKFLOW only.
        // `mcp_tool_calls.workflow_run_id` FKs `workflow_runs`; a CHAT `run_id` is
        // the assistant message id, not a workflow run, so setting it would
        // FK-violate the insert (and silently drop the recording). The chat row is
        // instead owner/conversation-scoped via the session context.
        if matches!(
            source,
            crate::modules::mcp::tool_calls::models::McpToolCallSource::Workflow
        ) {
            guard.set_workflow_run(scope.run_id);
            // ITEM-16: the idempotency key is persisted as the row's `tool_use_id`
            // (unused for a workflow call). A chat call's real tool_use_id comes
            // from the LLM ContentBlock, so don't overwrite it with the key here.
            if let Some(key) = idempotency_key {
                guard.set_idempotency_key(key);
            }
        }
        // ITEM-12: carry the reviewer classification onto the journal row.
        guard.set_review_classification(review_classification);
        guard.call_tool(tool_name, args, None, None, None).await
    };
    let result = tokio::select! {
        r = call => r,
        _ = cancel.cancelled() => return Err(McpToolCallError::Cancelled),
    };
    match result {
        Ok(r) => Ok((server_id, r)),
        Err(e) => Err(McpToolCallError::Failed(format!("tool '{tool_name}': {e}"))),
    }
}

/// Split a namespaced tool wire name `<server>__<tool>` into (server, tool).
/// A bare name (no `__`) yields an empty server + the whole string. Shared by
/// both agent hosts (was copy-pasted in `chat::agent_host::resolver` and
/// `workflow::agent_dispatch`).
pub(crate) fn split_tool_name(name: &str) -> (String, String) {
    match name.find("__") {
        Some(idx) => (name[..idx].to_string(), name[idx + 2..].to_string()),
        None => (String::new(), name.to_string()),
    }
}

/// Flatten an MCP `ToolResult` into an `agent_core::ToolResult`: concatenate its
/// text blocks into one `Text` block, preserving `is_error` + `structured_content`.
///
/// `terminal` is the audience-only bypass (parity with the MCP extension's
/// `execute_tool`): if any content block is annotated `audience: ["user"]`
/// EXACTLY, the output is for the user, not the model — the turn ends with it (no
/// continuation round). Both hosts share this ONE definition; the workflow host
/// previously hardcoded `terminal: false`, which IGNORED the audience-terminal
/// signal (a latent bug) — unified here on the correct audience-computed value.
pub(crate) fn mcp_to_agent_result(
    r: crate::modules::mcp::client::traits::ToolResult,
) -> ToolResult {
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
    // No text blocks (e.g. an image-only or structured-only result) -> stringify
    // the raw content so the model still sees something actionable.
    if text.is_empty() && !r.content.is_empty() {
        text = serde_json::to_string(&r.content).unwrap_or_default();
    }
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
