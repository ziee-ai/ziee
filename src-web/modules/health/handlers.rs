// Health handlers

use axum::{http::StatusCode, Json};

use super::types::HealthResponse;

// =====================================================
// Route Handlers
// =====================================================

/// GET /api/health
/// Health check endpoint
pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
        }),
    )
}
