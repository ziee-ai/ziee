//! Auth Routes
//!
//! Route definitions for desktop authentication

use super::handlers;
use ziee_chat::{post, Router};

/// Create auth routes
pub fn auth_routes() -> Router {
    Router::new()
        .route("/api/desktop/auth/auto-login", post(handlers::desktop_auto_login))
}
