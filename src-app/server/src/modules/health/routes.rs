// Health routes configuration

use aide::axum::{routing::get_with, ApiRouter};

use super::handlers::*;

pub fn routes() -> ApiRouter {
    ApiRouter::new().api_route("/health", get_with(health_check, health_check_docs))
}
