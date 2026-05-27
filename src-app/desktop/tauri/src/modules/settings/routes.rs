//! Settings Routes
//!
//! Route definitions for desktop settings management

use aide::axum::ApiRouter;
use super::handlers;
use ziee::{delete_with, get_with, put_with};

/// Create settings API routes with OpenAPI documentation
pub fn settings_api_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/api/desktop/settings",
            get_with(handlers::get_all_settings, handlers::get_all_settings_docs),
        )
        .api_route(
            "/api/desktop/settings/{key}",
            get_with(handlers::get_setting, handlers::get_setting_docs),
        )
        .api_route(
            "/api/desktop/settings/{key}",
            put_with(handlers::set_setting, handlers::set_setting_docs),
        )
        .api_route(
            "/api/desktop/settings/{key}",
            delete_with(handlers::delete_setting, handlers::delete_setting_docs),
        )
}
