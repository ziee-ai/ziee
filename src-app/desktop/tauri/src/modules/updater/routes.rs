//! Updater Routes
//!
//! Route definitions for application update management

use super::handlers;
use ziee_chat::{get, post, Router};

/// Create updater routes
pub fn updater_routes() -> Router {
    Router::new()
        .route("/api/desktop/updater/check", post(handlers::check_for_updates))
        .route("/api/desktop/updater/download", post(handlers::download_update))
        .route("/api/desktop/updater/install", post(handlers::install_update))
        .route("/api/desktop/updater/status", get(handlers::get_update_status))
}
