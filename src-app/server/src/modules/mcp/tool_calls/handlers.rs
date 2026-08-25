//! REST handlers for the MCP tool-call history (`/api/mcp/tool-calls`).
//!
//! Mirrors `handlers/user.rs::list_accessible_servers`: gated on
//! `McpServersRead` (held by every Users-group member), owner-scoped on
//! `auth.user.id`. Cross-user single-row access returns 404 (MCP convention).

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::super::permissions::{McpServersAdminEdit, McpServersRead};
use super::models::{McpToolCall, McpToolCallListResponse, McpToolCallReveal};

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

/// Query params for the tool-call history list.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListToolCallsQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter to a single MCP server.
    #[serde(default)]
    pub server_id: Option<Uuid>,
    /// Filter to a single conversation.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    /// Filter by built-in vs external servers (e.g. `false` to hide built-ins).
    #[serde(default)]
    pub is_built_in: Option<bool>,
    /// Filter to the single call the model stamped with this `tool_use` id — the
    /// join key between an assistant message's `tool_use` block and its recorded
    /// invocation (duration / source / result size).
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// Filter to every call recorded under one assistant message.
    #[serde(default)]
    pub message_id: Option<Uuid>,
}

/// GET /api/mcp/tool-calls — the caller's own tool-call history, newest-first.
#[debug_handler]
pub async fn list_tool_calls(
    auth: RequirePermissions<(McpServersRead,)>,
    Query(params): Query<ListToolCallsQuery>,
) -> ApiResult<Json<McpToolCallListResponse>> {
    let response = Repos
        .mcp
        .list_tool_calls(
            auth.user.id,
            super::repository::ToolCallFilters {
                server_id: params.server_id,
                conversation_id: params.conversation_id,
                is_built_in: params.is_built_in,
                // Bound as `tool_use_id = $5` — same text-bind class as the
                // list `search` filters, same shared guard.
                //
                // `guard_raw`, NOT `normalize_text_filter`: this binds the RAW
                // value, so blank→None would widen `?tool_use_id=` from an
                // empty page to the caller's ENTIRE tool-call history.
                tool_use_id: crate::common::text_guard::guard_raw(
                    params.tool_use_id.as_deref(),
                    "tool_use_id",
                )?,
                message_id: params.message_id,
            },
            params.page.max(1),
            params.per_page,
        )
        .await?;
    Ok((StatusCode::OK, Json(response)))
}

pub fn list_tool_calls_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpToolCall.list")
        .tag("MCP Servers - Tool Calls")
        .summary("List MCP tool-call history")
        .description(
            "List the caller's own MCP tool-call invocations, newest-first. \
             Optional `server_id` / `conversation_id` / `is_built_in` / \
             `tool_use_id` / `message_id` filters. Owner-scoped: every filter \
             narrows the caller's own rows and can never reach another user's.",
        )
        .response::<200, Json<McpToolCallListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<400, (), _>(|res| {
            res.description("Invalid query parameter (e.g. a NUL byte in a free-text filter)")
        })
}

/// GET /api/mcp/tool-calls/{id} — one tool-call row (404 if not owned).
#[debug_handler]
pub async fn get_tool_call(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<McpToolCall>> {
    // Owner-scoped in SQL: a row owned by another user comes back None → 404.
    let row = Repos
        .mcp
        .get_tool_call(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Tool call"))?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_tool_call_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpToolCall.get")
        .tag("MCP Servers - Tool Calls")
        .summary("Get an MCP tool-call record")
        .description(
            "Fetch a single tool-call record by id. Owner-scoped: a row owned \
             by another user returns 404.",
        )
        .response::<200, Json<McpToolCall>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Not found"))
}

/// Emit the audit record for a SUCCESSFUL reveal (TEST-42).
///
/// Deliberately logs WHO revealed WHICH call, and deliberately does NOT log the
/// revealed value — an audit line that reprints the secret would defeat the
/// redaction it exists to account for. Extracted from the handler as a free
/// function purely so the emitted record is directly assertable (the handler
/// itself needs a live `Repos` + an authenticated extractor).
fn audit_reveal(
    user_id: Uuid,
    tool_call_id: Uuid,
    tool_name: &str,
    server_name: &str,
    raw_available: bool,
) {
    tracing::info!(
        target: "mcp::tool_calls::reveal",
        user_id = %user_id,
        tool_call_id = %tool_call_id,
        tool_name = %tool_name,
        server_name = %server_name,
        raw_available,
        "mcp: raw tool-call arguments revealed"
    );
}

/// GET /api/mcp/tool-calls/{id}/reveal — the RAW, unredacted arguments for one
/// recorded call (ITEM-17 / DEC-1: "redact by default, admin-gated reveal").
///
/// # Where the raw value actually lives
///
/// NOT in `mcp_tool_calls.arguments_json`: `record::cap_arguments` redacts BEFORE
/// the insert, so that column has never held the raw value and a reveal reading it
/// would just echo `[redacted]`. The raw arguments live on the paired
/// `message_contents` `tool_use` block's `input`.
///
/// # Two independent scopes
///
/// 1. **Permission** — `mcp_servers_admin::edit`. DEC-2 names "`mcp_servers::manage`",
///    which does not exist in this codebase; `McpServersAdminEdit` is the constant
///    that carries the capability DEC-2 describes (its holder can already read and
///    set a system server's configured secret headers), so revealing arguments
///    grants them nothing they lack. No new permission, no migration.
/// 2. **Ownership** — the row is resolved with `find_call_for_user` (another
///    user's call → 404 even for an admin), and the transcript block is then
///    reached only through `conversations.user_id = <caller>`. An admin therefore
///    reveals only their OWN calls; this is a "see past your own redaction"
///    affordance for the operator debugging a failing call, not a cross-user
///    inspection tool.
///
/// # Residual limitation — stated plainly, not overstated
///
/// The raw `tool_use.input` is ALSO present in the conversation-messages API
/// payload the owner already receives on every page load. This gate therefore
/// hides the value from the SURFACE (which is exactly what DEC-1 asks for: the
/// rail step and the detail panel render redacted by default) rather than removing
/// it from the wire. It is a surface-exposure control, NOT a containment boundary
/// against the owner. Closing the wire hole would mean redacting `tool_use.input`
/// in the message payload itself, which is out of scope here (and would need its
/// own reveal path for the transcript).
#[debug_handler]
pub async fn reveal_tool_call_arguments(
    auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<McpToolCallReveal>> {
    // Owner-scoped in SQL: another user's row comes back None → 404, admin or not.
    let row = Repos
        .mcp
        .get_tool_call(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Tool call"))?;

    // The raw value only exists when the call was stamped with BOTH the assistant
    // message and the model's tool_use id (the chat path). A REST/workflow call has
    // no transcript block, so it falls back to the recorded arguments.
    let raw = match (row.message_id, row.tool_use_id.as_deref()) {
        (Some(message_id), Some(tool_use_id)) => {
            Repos
                .mcp
                .get_raw_tool_use_input(message_id, tool_use_id, auth.user.id)
                .await?
        }
        _ => None,
    };

    audit_reveal(
        auth.user.id,
        id,
        &row.tool_name,
        &row.server_name,
        raw.is_some(),
    );

    let (arguments_json, is_raw) = match raw {
        Some(v) => (v, true),
        // Block gone (message deleted / branch pruned) → return the recorded,
        // already-redacted arguments rather than erroring, so the panel degrades
        // instead of breaking.
        None => (row.arguments_json.clone(), false),
    };

    Ok((
        StatusCode::OK,
        Json(McpToolCallReveal {
            id: row.id,
            arguments_json,
            raw: is_raw,
        }),
    ))
}

pub fn reveal_tool_call_arguments_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpToolCall.reveal")
        .tag("MCP Servers - Tool Calls")
        .summary("Reveal a tool call's raw arguments")
        .description(
            "Return the RAW, unredacted arguments for one recorded tool call, read \
             from the paired transcript `tool_use` block (the stored \
             `arguments_json` column is redacted before insert and never held the \
             raw value). Requires `mcp_servers_admin::edit` AND ownership of the \
             call: another user's row returns 404. When the transcript block no \
             longer exists the recorded (redacted) arguments are returned with \
             `raw: false`.",
        )
        .response::<200, Json<McpToolCallReveal>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Missing `mcp_servers_admin::edit`"))
        .response_with::<404, (), _>(|res| {
            res.description("No such tool call, or it is not owned by the caller")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A `MakeWriter` that accumulates the formatted subscriber output so a test
    /// can assert on the REAL emitted record (not a paraphrase of it).
    #[derive(Clone)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// TEST-42 (audit clause): a successful reveal is RECORDED, naming the acting
    /// user and the call — and NOT the revealed value.
    ///
    /// Asserted against the actually-rendered `tracing` output, by installing a
    /// capturing subscriber around the real emission site. (The integration test
    /// cannot do this: the harness spawns the server as a subprocess with
    /// inherited stdio, so its log stream is not readable from the test.)
    #[test]
    fn reveal_audit_names_the_actor_and_the_call_but_never_the_value() {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let writer_buf = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(move || CaptureWriter(writer_buf.clone()))
            .finish();

        let user_id = Uuid::from_u128(0x1111_2222);
        let call_id = Uuid::from_u128(0x3333_4444);
        tracing::subscriber::with_default(subscriber, || {
            audit_reveal(user_id, call_id, "call_api", "acme-remote", true);
        });

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.contains("mcp: raw tool-call arguments revealed"),
            "an audit line must be emitted: {out}"
        );
        assert!(
            out.contains(&user_id.to_string()),
            "the audit line must name the acting user: {out}"
        );
        assert!(
            out.contains(&call_id.to_string()),
            "the audit line must name the revealed call: {out}"
        );
        assert!(out.contains("call_api"), "the tool is named: {out}");
        assert!(out.contains("acme-remote"), "the server is named: {out}");
        assert!(
            out.contains("raw_available=true"),
            "whether a raw value was actually available is recorded: {out}"
        );
        // The signature makes it structurally impossible to log the value; this
        // pins that no FIELD carrying it is ever added. (The human-readable
        // message legitimately contains the word "arguments", so the guard is on
        // the field names — `key=value` — not on the prose.)
        for forbidden in ["arguments=", "arguments_json=", "input=", "value="] {
            assert!(
                !out.contains(forbidden),
                "the audit line must never carry a `{forbidden}` field: {out}"
            );
        }
    }
}
