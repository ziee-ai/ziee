// Onboarding routes

use aide::axum::{ApiRouter, routing::post_with};

use super::handlers::*;

pub fn onboarding_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/onboarding/{guide_id}/complete",
            post_with(complete_guide, complete_guide_docs),
        )
        .api_route(
            "/onboarding/{guide_id}/steps/{step_id}/complete",
            post_with(complete_guide_step, complete_guide_step_docs),
        )
}
