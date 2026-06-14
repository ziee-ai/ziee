//! JSON-RPC handler for the built-in skill_mcp server.
//!
//! Mirrors `memory_mcp/handlers.rs` shape (reuses JSON-RPC envelope
//! types from `code_sandbox::types`). Both tools are read-only, so
//! `skills::read` is the only permission gate; no per-tool authorization
//! split.

#![allow(dead_code)]

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::RequirePermissions;
use crate::modules::skill::permissions::SkillsRead;

use super::tools;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(SkillsRead,)>,
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

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "skill",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "tools/list" => ok_response(id, tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => match dispatch_tool_call(auth.user.id, conversation_id, &req.params).await
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

    let result = match call.name.as_str() {
        "load_skill" => tools::load_skill(user_id, conversation_id, &call.arguments).await,
        "read_skill_file" => {
            tools::read_skill_file(user_id, conversation_id, &call.arguments).await
        }
        other => {
            return Err((
                StatusCode::OK,
                JsonRpcError::method_not_found(&format!("skill tool: {other}")),
            ));
        }
    };

    match result {
        Ok(v) => Ok(json!({
            "content": [{ "type": "text", "text": v.to_string() }],
            "structuredContent": v,
        })),
        Err(e) => Err((StatusCode::OK, app_error_to_jsonrpc(&e))),
    }
}

fn app_error_to_jsonrpc(e: &AppError) -> JsonRpcError {
    JsonRpcError::from_app_error(e)
}
