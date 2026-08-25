//! JSON-RPC MCP endpoint for `get_tool_result`.
//!
//! Gated on `profile::read` (the JWT is validated by the extractor — every
//! authenticated user has it). The result is scoped to a conversation the caller
//! OWNS (verified via `get_conversation_user_id`), so the model can only recall
//! tool results from its own conversation — data it already received.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::mcp::permissions::McpServersRead;
use crate::modules::permissions::RequirePermissions;

pub async fn jsonrpc_handler(
    auth: RequirePermissions<(McpServersRead,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::parse_error(e.to_string()),
            );
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::invalid_request(e.to_string()),
            );
        }
    };

    // Notifications carry no `id`, expect no response.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "tool_result", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => {
            match dispatch_tool_call(auth.user.id, conversation_id, &req.params).await {
                Ok(value) => ok_response(id, value),
                Err(e) => error_response(id, e.0, e.1),
            }
        }
        _ => error_response(id, StatusCode::OK, JsonRpcError::method_not_found(&req.method)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

async fn dispatch_tool_call(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    params: &Value,
) -> Result<Value, (StatusCode, JsonRpcError)> {
    let call: ToolCallParams = serde_json::from_value(params.clone()).map_err(|e| {
        (
            StatusCode::OK,
            JsonRpcError::invalid_params(format!("tools/call params: {e}")),
        )
    })?;
    if call.name != "get_tool_result" {
        return Err((
            StatusCode::OK,
            JsonRpcError::method_not_found(&format!("tool_result tool: {}", call.name)),
        ));
    }
    get_tool_result(user_id, conversation_id, &call.arguments)
        .await
        .map_err(|e| (StatusCode::OK, JsonRpcError::from_app_error(&e)))
}

#[derive(Debug, Deserialize)]
struct GetArgs {
    tool_use_id: String,
    #[serde(default)]
    offset: Option<i64>,
    #[serde(default)]
    max_chars: Option<i64>,
}

/// Default per-call recall window — matches the kept-tool-result ceiling so a
/// recalled result is itself trim-friendly working memory.
const DEFAULT_MAX_CHARS: i64 = 8000;

async fn get_tool_result(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let args: GetArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let tuid = args.tool_use_id.trim();
    if tuid.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "tool_use_id must not be empty",
        ));
    }
    let conversation_id = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "NO_CONVERSATION",
            "get_tool_result requires a conversation context",
        )
    })?;

    // Ownership: the conversation must belong to the caller. A spoofed
    // x-conversation-id (or one the user doesn't own) yields NOT_FOUND, so this
    // can't be used to read another user's tool results.
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(conversation_id)
        .await
        .ok()
        .flatten();
    if owner != Some(user_id) {
        return Err(AppError::bad_request(
            "NOT_FOUND",
            "no such tool result in this conversation",
        ));
    }

    // Look up the persisted tool_result content block by tool_use_id, scoped to
    // the (owned) conversation via branch_messages → branches.
    //
    // NOTE: this is conversation-scoped but branch-AGNOSTIC — it matches the
    // tool_use_id on ANY branch of the conversation (a `tool_use_id` is unique
    // within a conversation in practice), taking the most recent on a tie. Exact
    // recall of the persisted result doesn't depend on the active branch, so this
    // is intentional; branch-accurate recall would join on `active_branch_id`.
    let block: Option<Value> = sqlx::query_scalar!(
        r#"
        SELECT mc.content AS "content!: Value"
        FROM message_contents mc
        JOIN branch_messages bm ON bm.message_id = mc.message_id
        JOIN branches b ON b.id = bm.branch_id
        WHERE b.conversation_id = $1
          AND mc.content_type = 'tool_result'
          AND mc.content->>'tool_use_id' = $2
        ORDER BY mc.created_at DESC
        LIMIT 1
        "#,
        conversation_id,
        tuid
    )
    .fetch_optional(Repos.pool())
    .await
    .map_err(AppError::database_error)?;

    let Some(block) = block else {
        return Err(AppError::bad_request(
            "NOT_FOUND",
            format!("no tool result with tool_use_id '{tuid}' in this conversation"),
        ));
    };

    // Build the full recall text: the persisted `content` text + (when present)
    // the structuredContent serialized — this is the ONLY way the model can read
    // structuredContent (to_content_block never forwards it).
    let mut full = block
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(sc) = block.get("structured_content").filter(|v| !v.is_null()) {
        let pretty = serde_json::to_string_pretty(sc).unwrap_or_else(|_| sc.to_string());
        full.push_str(&format!("\n\n--- structuredContent ---\n{pretty}"));
    }

    // Page over chars (char-safe — never split a multibyte sequence).
    let offset = args.offset.unwrap_or(0).max(0) as usize;
    let max_chars = args.max_chars.unwrap_or(DEFAULT_MAX_CHARS).clamp(1, 100_000) as usize;
    let page = page_chars(&full, offset, max_chars, tuid);

    Ok(json!({
        "content": [{ "type": "text", "text": page.text }],
        "structuredContent": {
            "tool_use_id": tuid,
            "total_chars": page.total_chars,
            "offset": offset,
            "returned_chars": page.returned,
            "has_more": page.has_more,
        },
    }))
}

/// Outcome of paging `full` into a window starting at `offset`.
struct Page {
    /// The window's text, with a continuation marker appended when `has_more`.
    text: String,
    /// Chars actually returned (before the marker).
    returned: usize,
    /// Total chars in `full`.
    total_chars: usize,
    has_more: bool,
}

/// Pure char-window paging over `full` (char-safe — never splits a multibyte
/// sequence). When more remains, appends a marker pointing at the next offset.
///
/// Iterates `char_indices` once (no full `Vec<char>` materialization) so paging a
/// ~1 MB recall in small windows doesn't re-allocate the whole char vector per
/// page — just locates the window's byte bounds and slices.
fn page_chars(full: &str, offset: usize, max_chars: usize, tuid: &str) -> Page {
    // Byte offset where the window starts (offset-th char) and ends (offset+max_chars).
    let mut start_byte = full.len();
    let mut end_byte = full.len();
    let mut total_chars = 0usize;
    for (ci, (bi, _)) in full.char_indices().enumerate() {
        if ci == offset {
            start_byte = bi;
        }
        if ci == offset + max_chars {
            end_byte = bi;
        }
        total_chars = ci + 1;
    }
    if offset >= total_chars {
        start_byte = full.len();
        end_byte = full.len();
    }
    let mut text = full[start_byte..end_byte].to_string();
    let returned = text.chars().count();
    let has_more = offset + returned < total_chars;
    if has_more {
        let next = offset + returned;
        text.push_str(&format!(
            "\n[… showing chars {offset}–{next} of {total_chars}; more available — call get_tool_result(tool_use_id=\"{tuid}\", offset={next}) …]"
        ));
    }
    Page { text, returned, total_chars, has_more }
}

#[cfg(test)]
mod tests {
    use super::page_chars;

    #[test]
    fn first_window_has_more() {
        let p = page_chars("abcdefghij", 0, 4, "tu1");
        assert_eq!(p.returned, 4);
        assert_eq!(p.total_chars, 10);
        assert!(p.has_more);
        assert!(p.text.starts_with("abcd"));
        assert!(p.text.contains("offset=4"));
    }

    #[test]
    fn last_window_no_more_and_no_marker() {
        let p = page_chars("abcdefghij", 6, 100, "tu1");
        assert_eq!(p.returned, 4);
        assert!(!p.has_more);
        assert_eq!(p.text, "ghij");
    }

    #[test]
    fn offset_exactly_at_end_is_empty_no_more() {
        let p = page_chars("abcde", 5, 10, "tu1");
        assert_eq!(p.returned, 0);
        assert!(!p.has_more);
        assert_eq!(p.text, "");
    }

    #[test]
    fn offset_past_end_is_empty_no_more() {
        let p = page_chars("abcde", 99, 10, "tu1");
        assert_eq!(p.returned, 0);
        assert!(!p.has_more);
        assert_eq!(p.total_chars, 5);
    }

    #[test]
    fn multibyte_is_not_split() {
        // 5 multibyte chars; a 3-char window must return whole chars.
        let p = page_chars("αβγδε", 0, 3, "tu1");
        assert_eq!(p.returned, 3);
        assert_eq!(p.total_chars, 5);
        assert!(p.has_more);
        assert!(p.text.starts_with("αβγ"));
    }
}
