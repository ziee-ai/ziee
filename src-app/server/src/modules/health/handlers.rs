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

    /// The health endpoint handler returns 200 with a static `{"status":"ok"}`
    /// body. The health module had zero unit tests.
    #[tokio::test]
    async fn health_check_returns_ok_status() {
        let (status, Json(body)) = health_check().await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
    }

    /// HealthResponse serializes to the documented JSON shape.
    #[test]
    fn health_response_serializes_to_status_field() {
        let body = HealthResponse { status: "ok".to_string() };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }
}
