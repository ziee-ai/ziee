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

// =====================================================================
// Admin-configurable limits (js_tool_settings) — GET/PUT /api/js-tool/settings.
// Mirrors code_sandbox's resource-limits handlers.
// =====================================================================

use crate::core::repository::Repos;
use crate::modules::js_tool::permissions::{JsToolSettingsManage, JsToolSettingsRead};
use crate::modules::js_tool::settings::{JsToolSettings, UpdateJsToolSettings};
use crate::modules::permissions::openapi::with_permission;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

/// GET /js-tool/settings — read the singleton run_js limits (admin-only).
pub async fn get_settings_handler(
    _auth: RequirePermissions<(JsToolSettingsRead,)>,
) -> crate::common::ApiResult<Json<JsToolSettings>> {
    let row = Repos.js_tool.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(JsToolSettingsRead,)>(op)
        .id("JsTool.getSettings")
        .tag("Programmatic Tools")
        .summary("Read the singleton run_js (js_tool) limits configuration")
        .description(
            "Returns the current memory / stack / wall-clock / approval-timeout / \
             concurrency / trace caps applied on every run_js invocation. Admin-only.",
        )
        .response::<200, Json<JsToolSettings>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

/// PUT /js-tool/settings — update the singleton run_js limits (admin-only).
pub async fn update_settings_handler(
    _auth: RequirePermissions<(JsToolSettingsManage,)>,
    origin: SyncOrigin,
    Json(patch): Json<UpdateJsToolSettings>,
) -> crate::common::ApiResult<Json<JsToolSettings>> {
    patch.validate()?;
    let row = Repos.js_tool.update_settings(&patch).await?;
    // Invalidate the cached snapshot the run_js hot path reads from so the new
    // caps take effect on the very next invocation, and live-resize the global
    // admission semaphore (max_concurrent_runs) immediately.
    crate::modules::js_tool::settings_cache::invalidate(&row);
    crate::modules::js_tool::executor::set_max_concurrent_runs(row.max_concurrent_runs.max(1) as usize);
    sync_publish(
        SyncEntity::JsToolSettings,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<JsToolSettingsRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(JsToolSettingsManage,)>(op)
        .id("JsTool.updateSettings")
        .tag("Programmatic Tools")
        .summary("Update the singleton run_js (js_tool) limits configuration")
        .description(
            "PATCH semantics: any field omitted from the body preserves its existing \
             value. Validation rejects out-of-range values with 422; the new caps take \
             effect on the very next run_js invocation.",
        )
        .response::<200, Json<JsToolSettings>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<422, (), _>(|r| r.description("Value out of range"))
}
