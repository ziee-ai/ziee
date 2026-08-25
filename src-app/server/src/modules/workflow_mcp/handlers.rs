//! JSON-RPC handler for the built-in workflow_mcp server.
//!
//! Mirrors `skill_mcp/handlers.rs` / `memory_mcp/handlers.rs` shape
//! (reuses the JSON-RPC envelope types from `code_sandbox::types`).
//! Auth is gated on `workflows::execute` (the run action). The
//! conversation is resolved from the `x-conversation-id` header; the
//! user from the short-lived JWT the MCP client injects for built-in
//! servers.
//!
//! Methods:
//! - `initialize` / `ping`
//! - `tools/list` — one `wf_<slug>` tool per accessible workflow
//! - `tools/call` — spawn the runner, await completion, format result
//! - `resources/list` / `resources/read` — outputs / artifacts / logs


use axum::{
    Json, debug_handler,
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
use crate::modules::permissions::RequirePermissions;
use crate::modules::workflow::permissions::WorkflowsExecute;

use super::resources;
use super::tools;
use super::workflow_mcp_server_id;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(WorkflowsExecute,)>,
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
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": {
                    "name": "workflow",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "ping" => ok_response(id, json!({})),
        "tools/list" => match tools::tool_list(pool, user_id).await {
            Ok(v) => ok_response(id, v),
            Err(e) => error_response(id, StatusCode::OK, JsonRpcError::from_app_error(&e)),
        },
        "tools/call" => {
            match dispatch_tool_call(pool, user_id, conversation_id, &req.params).await {
                Ok(value) => ok_response(id, value),
                Err((http, err)) => error_response(id, http, err),
            }
        }
        "resources/list" => match resources::resources_list(pool, user_id).await {
            Ok(v) => ok_response(id, v),
            Err(e) => error_response(id, StatusCode::OK, JsonRpcError::from_app_error(&e)),
        },
        "resources/read" => {
            match dispatch_resources_read(pool, user_id, &req.params).await {
                Ok(v) => ok_response(id, v),
                Err((http, err)) => error_response(id, http, err),
            }
        }
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

    // The client may send either the bare leaf (`wf_<slug>`) or the
    // composed `<server_uuid>__wf_<slug>`. Strip the composed prefix if
    // present so the tool dispatcher always sees the leaf.
    let prefix = format!("{}__", workflow_mcp_server_id());
    let leaf = call.name.strip_prefix(&prefix).unwrap_or(&call.name);

    tools::call_tool(pool, user_id, conversation_id, leaf, &call.arguments)
        .await
        .map_err(|e| (StatusCode::OK, JsonRpcError::from_app_error(&e)))
}

#[derive(Debug, Deserialize)]
struct ResourcesReadParams {
    uri: String,
}

async fn dispatch_resources_read(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    params: &Value,
) -> Result<Value, (StatusCode, JsonRpcError)> {
    let p: ResourcesReadParams = serde_json::from_value(params.clone()).map_err(|e| {
        (
            StatusCode::OK,
            JsonRpcError::invalid_params(format!("resources/read params: {e}")),
        )
    })?;
    resources::resources_read(pool, user_id, &p.uri)
        .await
        .map_err(|e| (StatusCode::OK, app_error_to_jsonrpc(&e)))
}

fn app_error_to_jsonrpc(e: &AppError) -> JsonRpcError {
    JsonRpcError::from_app_error(e)
}
