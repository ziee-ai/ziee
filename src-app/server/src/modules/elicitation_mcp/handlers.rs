//! JSON-RPC handler for the built-in elicitation MCP server.
//!
//! Serves `initialize` + `tools/list` + `ping` so the `ask_user` tool is
//! discoverable and the MCP session connects. `tools/call` is a NEVER-HIT
//! fallback: the chat tool-loop intercepts `ask_user` and drives the
//! elicitation inline (it needs the live chat `sse_tx`, which a loopback
//! handler can't reach). The fallback returns an error so an out-of-loop
//! invocation fails loudly rather than silently no-op'ing.

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::modules::code_sandbox::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::modules::mcp::permissions::McpServersRead;
use crate::modules::permissions::RequirePermissions;

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `mcp_servers::read` — the same permission the elicitation
    // /respond endpoint requires, so any user who can answer can also
    // discover the tool. Authentication still runs for every call.
    _auth: RequirePermissions<(McpServersRead,)>,
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
                "serverInfo": {
                    "name": "elicitation",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        // The chat tool-loop intercepts `ask_user` before any loopback
        // dispatch; reaching here means it was invoked outside an
        // interactive chat stream, where there's no user to ask.
        "tools/call" => ok_response(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": "ask_user can only run inside an interactive chat turn (no user form channel is available here)."
                }],
                "isError": true,
            }),
        ),
        _ => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::method_not_found(&req.method),
        ),
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
