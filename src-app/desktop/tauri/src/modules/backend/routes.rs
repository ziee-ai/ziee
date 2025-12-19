//! Backend Routes
//!
//! Route definitions for backend management

use super::handlers;
use ziee_chat::{get, Router};

/// Create backend routes
pub fn backend_routes() -> Router {
    Router::new()
        .route("/api/desktop/backend/status", get(handlers::get_backend_status))
}
