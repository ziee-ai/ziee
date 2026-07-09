//! JSON-RPC handler for the built-in `run_js` MCP server.
//!
//! This endpoint exists so `run_js` is discoverable in the normal way:
//! `initialize` + `tools/list` are served here (the tool assembly in
//! `mcp/chat_extension/mcp.rs` connects over loopback to list the descriptor).
//! `tools/call` is NOT executed here — a `run_js` call needs the live chat
//! context (session manager, accessible tools, `sse_tx`, approval channel), so
//! it is intercepted inline in `mcp.rs` before it would ever reach the loopback
//! (exactly like `ask_user`). If a `tools/call` somehow reaches here it returns
//! an explicit error rather than silently doing nothing.
//!
//! Reuses the JSON-RPC envelope from `code_sandbox` (no duplicate scaffolding).

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::js_tool::permissions::JsToolUse;
use crate::modules::permissions::RequirePermissions;

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `js_tool::use` so authentication runs for every call.
    _auth: RequirePermissions<(JsToolUse,)>,
    ConversationIdHeader(_conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(None, StatusCode::BAD_REQUEST, JsonRpcError::parse_error(e.to_string()));
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(None, StatusCode::BAD_REQUEST, JsonRpcError::invalid_request(e.to_string()));
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
                "serverInfo": { "name": "run_js", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::invalid_request(
                "run_js must be invoked in a chat context; it is executed inline by the chat runtime, not over the loopback transport",
            ),
        ),
        _ => error_response(id, StatusCode::OK, JsonRpcError::method_not_found(&req.method)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse { jsonrpc: "2.0", id, result: Some(result), error: None }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse { jsonrpc: "2.0", id, result: None, error: Some(err) }),
    )
        .into_response()
}
