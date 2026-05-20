// Elicitation routes

use aide::axum::{ApiRouter, routing::post_with};

use super::handlers::{respond_to_elicitation, respond_to_elicitation_docs};

pub fn elicitation_routes() -> ApiRouter {
    ApiRouter::new().api_route(
        "/mcp/elicitation/{elicitation_id}/respond",
        post_with(respond_to_elicitation, respond_to_elicitation_docs),
    )
}
