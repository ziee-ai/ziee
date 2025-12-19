//! Auth Routes
//!
//! Route definitions for desktop authentication
//!
//! NOTE: Auto-login has been moved to a Tauri command (see commands.rs)
//! to prevent web-based clients from accessing it.

use aide::axum::ApiRouter;

/// Create auth API routes with OpenAPI documentation
///
/// Returns empty router - auto-login is now a Tauri command
pub fn auth_api_routes() -> ApiRouter {
    ApiRouter::new()
}
