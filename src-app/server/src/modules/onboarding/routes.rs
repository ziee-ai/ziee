// Onboarding routes

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};

use super::handlers::*;

pub fn onboarding_router() -> ApiRouter {
    ApiRouter::new()
        // Literal route registered before the `{guide_id}` param routes.
        .api_route(
            "/onboarding/progress",
            get_with(get_progress, get_progress_docs),
        )
        .api_route(
            "/onboarding/{guide_id}/complete",
            post_with(complete_guide, complete_guide_docs),
        )
        .api_route(
            "/onboarding/{guide_id}/steps/{step_id}/complete",
            post_with(complete_guide_step, complete_guide_step_docs),
        )
}
