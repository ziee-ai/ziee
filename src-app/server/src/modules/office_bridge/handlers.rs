//! HTTP handlers: the JSON-RPC MCP endpoint + the admin settings REST surface.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::models::{OfficeBridgeSettings, UpdateOfficeBridgeSettingsRequest};
use super::permissions::{OfficeBridgeAdminRead, OfficeBridgeManage, OfficeBridgeUse};

// ─────────────────────────── JSON-RPC MCP endpoint ───────────────────────────

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `office_bridge::use`; the JWT is validated by the extractor.
    // Conversation id is accepted but unused in this increment.
    _auth: RequirePermissions<(OfficeBridgeUse,)>,
    ConversationIdHeader(_conversation_id): ConversationIdHeader,
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
                "serverInfo": { "name": "office_bridge", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        // Real tool dispatch (list_open_documents + the read/edit/comment/
        // track-changes tools) is wired in ITEM-9. Until then, surface a
        // client-class JSON-RPC error rather than pretending to act.
        "tools/call" => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::internal(
                "office_bridge tools are not yet implemented (dispatch lands in a later increment)",
            ),
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

// ─────────────────────────── Admin REST: settings ───────────────────────────

#[debug_handler]
pub async fn get_settings(
    _auth: RequirePermissions<(OfficeBridgeAdminRead,)>,
) -> ApiResult<Json<OfficeBridgeSettings>> {
    let row = Repos.office_bridge.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeAdminRead,)>(op)
        .id("OfficeBridge.getSettings")
        .tag("OfficeBridge")
        .summary("Read office-bridge settings")
        .response::<200, Json<OfficeBridgeSettings>>()
}

#[debug_handler]
pub async fn update_settings(
    _auth: RequirePermissions<(OfficeBridgeManage,)>,
    Json(body): Json<UpdateOfficeBridgeSettingsRequest>,
) -> ApiResult<Json<OfficeBridgeSettings>> {
    if let Some(port) = body.port
        && !(1..=65535).contains(&port)
    {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "port out of range (1..=65535)").into(),
        );
    }

    let row = Repos
        .office_bridge
        .update_settings(body.enabled, body.port)
        .await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeManage,)>(op)
        .id("OfficeBridge.updateSettings")
        .tag("OfficeBridge")
        .summary("Update office-bridge settings (enable, port)")
        .response::<200, Json<OfficeBridgeSettings>>()
}
