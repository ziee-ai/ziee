// Health handlers

use aide::transform::TransformOperation;
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

/// Documentation for health_check endpoint
pub fn health_check_docs(op: TransformOperation) -> TransformOperation {
    op.description("Health check endpoint")
        .id("Health.check")
        .tag("health")
        .response::<200, Json<HealthResponse>>()
}
