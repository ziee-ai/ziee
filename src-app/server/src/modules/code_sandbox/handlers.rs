//! POST `/api/code-sandbox` + GET `/api/code-sandbox/file/download`.
//! Full dispatch / JWT validation / per-conversation mutex landed in Phase 5.

use axum::http::StatusCode;
use axum::Json;

use crate::modules::code_sandbox::types::{JsonRpcRequest, JsonRpcResponse};

/// JSON-RPC entry point. Stub for Phase 1 — returns an `internal` error
/// for every method until Phase 5 wires the real dispatcher.
pub async fn jsonrpc_handler(
    Json(req): Json<JsonRpcRequest>,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let response = JsonRpcResponse {
        jsonrpc: "2.0",
        id: req.id,
        result: None,
        error: Some(crate::modules::code_sandbox::types::JsonRpcError::internal(
            "code_sandbox handler not yet implemented",
        )),
    };
    (StatusCode::OK, Json(response))
}
