// Health handlers

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};

use super::types::HealthResponse;

// =====================================================
// Route Handlers
// =====================================================

/// GET /api/health
/// Health check endpoint
#[debug_handler]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The health endpoint is an unauthenticated liveness probe: it must always
    /// return 200 with `{"status":"ok"}` (no DB, no auth). Gap 6da0741c8b92 —
    /// the health module had zero unit tests.
    #[tokio::test]
    async fn health_check_returns_ok_200() {
        let (status, Json(body)) = health_check().await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
    }
}
