// Health routes configuration

use aide::axum::{routing::get_with, ApiRouter};
use axum::Json;

use super::handlers::*;
use super::types::HealthResponse;

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
