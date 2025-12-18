//! Settings Routes
//!
//! Route definitions for desktop settings management

use super::handlers;
use axum::routing::{delete, get, put};
use ziee_chat::Router;

/// Create settings routes
pub fn settings_routes() -> Router {
    Router::new()
        .route("/api/desktop/settings", get(handlers::get_all_settings))
        .route("/api/desktop/settings/{key}", get(handlers::get_setting))
        .route("/api/desktop/settings/{key}", put(handlers::set_setting))
        .route("/api/desktop/settings/{key}", delete(handlers::delete_setting))
}
