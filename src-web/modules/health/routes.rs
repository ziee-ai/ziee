use aide::axum::{routing::get_with, ApiRouter};
use axum::{http::StatusCode, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthResponse {
    pub status: String,
}

pub fn routes() -> ApiRouter {
    ApiRouter::new().api_route(
        "/health",
        get_with(health_check, |op| {
            op.description("Health check endpoint")
                .id("Health.check")
                .tag("health")
                .response::<200, Json<HealthResponse>>()
        }),
    )
}

async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
        }),
    )
}
