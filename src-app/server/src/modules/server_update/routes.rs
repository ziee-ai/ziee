use aide::axum::{ApiRouter, routing::get_with};

use super::handlers::*;

pub fn routes() -> ApiRouter {
    ApiRouter::new().api_route(
        "/status",
        get_with(get_update_status, get_update_status_docs),
    )
}
