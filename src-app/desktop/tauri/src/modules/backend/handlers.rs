//! Backend Handlers
//!
//! HTTP route handlers for backend management

use serde::Serialize;
use ziee_chat::Json;

/// Backend status response
#[derive(Serialize)]
pub struct BackendStatusResponse {
    pub running: bool,
    pub ready: bool,
    pub version: String,
}

/// Get backend status
pub async fn get_backend_status() -> Json<BackendStatusResponse> {
    Json(BackendStatusResponse {
        running: true,
        ready: true, // If this route responds, the server is ready
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
