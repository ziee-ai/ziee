//! Backend Handlers
//!
//! HTTP route handlers for backend management

use schemars::JsonSchema;
use serde::Serialize;
use ziee::{Json, TransformOperation};

/// Backend status response
#[derive(Serialize, JsonSchema)]
pub struct BackendStatusResponse {
    pub running: bool,
    pub ready: bool,
    pub version: String,
}

/// OpenAPI documentation for get_backend_status endpoint
pub fn get_backend_status_docs(op: TransformOperation) -> TransformOperation {
    op.description("Get backend server status")
        .id("DesktopBackend.status")
        .tag("desktop-backend")
        .response::<200, Json<BackendStatusResponse>>()
}

/// Get backend status
pub async fn get_backend_status() -> Json<BackendStatusResponse> {
    Json(BackendStatusResponse {
        running: true,
        ready: true, // If this route responds, the server is ready
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
