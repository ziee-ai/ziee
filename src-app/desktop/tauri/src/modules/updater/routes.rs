//! Updater Routes
//!
//! aide-documented route definitions for application update management.

use super::handlers;
use aide::axum::ApiRouter;
use ziee::{get_with, post_with};

/// Create updater API routes with OpenAPI documentation
pub fn updater_api_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/api/desktop/updater/check",
            post_with(handlers::check_for_updates, handlers::check_for_updates_docs),
        )
        .api_route(
            "/api/desktop/updater/download",
            post_with(handlers::download_update, handlers::download_update_docs),
        )
        .api_route(
            "/api/desktop/updater/install",
            post_with(handlers::install_update, handlers::install_update_docs),
        )
        .api_route(
            "/api/desktop/updater/status",
            get_with(handlers::get_update_status, handlers::get_update_status_docs),
        )
}
