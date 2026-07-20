//! JSON-RPC handler for the built-in background_mcp server.
//!
//! Mirrors `workflow_mcp/handlers.rs` / `memory_mcp/handlers.rs` shape (reuses
//! the JSON-RPC envelope types from `code_sandbox::types`). Auth is gated on
//! `background::use` (`RequirePermissions`). The conversation is resolved from
//! the `x-conversation-id` header; the user from the short-lived JWT the MCP
//! client injects for built-in servers.
//!
//! Methods:
//! - `initialize` / `ping`
//! - `tools/list` — the uniform `spawn_background` / `check_status` /
//!   `collect_result` trio
//! - `tools/call` — dispatch into `tools::call_tool`

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::RequirePermissions;

use super::background_mcp_server_id;
use super::permissions::BackgroundUse;
use super::tools;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(BackgroundUse,)>,
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

    // Notifications carry no `id`; expect no response.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let id = req.id.clone();
    let user_id = auth.user.id;
    let pool = Repos.pool();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "background",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "ping" => ok_response(id, json!({})),
        "tools/list" => ok_response(id, tools::tool_list()),
        "tools/call" => match dispatch_tool_call(pool, user_id, conversation_id, &req.params).await
        {
            Ok(value) => ok_response(id, value),
            Err((http, err)) => error_response(id, http, err),
        },
        _ => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::method_not_found(&req.method),
        ),
    }
}

/// Streamable-HTTP GET (session-open / server→client SSE). This in-process
/// built-in speaks JSON-RPC over POST only — it offers no server-initiated SSE
/// stream — so a GET is answered `405 Method Not Allowed`, exactly as the MCP
/// Streamable-HTTP spec permits ("the server MAY return 405 if it does not offer
/// an SSE stream at this endpoint"). The MCP client for built-in servers only
/// POSTs, so this path is never taken in practice; it exists so a probing client
/// gets a spec-correct answer rather than a bare 404.
#[debug_handler]
pub async fn jsonrpc_handler_get() -> Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
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
    pool: &sqlx::PgPool,
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

    // The client may send either the bare leaf (`spawn_background`) or the
    // composed `<server_uuid>__spawn_background`. Strip the composed prefix if
    // present so the tool dispatcher always sees the leaf.
    let prefix = format!("{}__", background_mcp_server_id());
    let leaf = call.name.strip_prefix(&prefix).unwrap_or(&call.name);

    let result = tools::call_tool(pool, user_id, conversation_id, leaf, &call.arguments)
        .await
        .map_err(|e| (StatusCode::OK, JsonRpcError::from_app_error(&e)))?;

    // MCP `tools/call` result envelope (mirrors memory_mcp): a text projection
    // for the model + the typed structuredContent for programmatic recall.
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "structuredContent": result,
    }))
}
