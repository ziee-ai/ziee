//! Agent module HTTP routes.

use aide::axum::{ApiRouter, routing::get_with};

use super::handlers::*;

pub fn agent_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/agent/settings",
        get_with(get_admin_settings, get_admin_settings_docs)
            .put_with(update_admin_settings, update_admin_settings_docs),
    )
}
