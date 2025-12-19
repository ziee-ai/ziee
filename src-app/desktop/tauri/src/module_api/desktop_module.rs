//! Desktop Module API
//!
//! Trait-based module system for Tauri desktop features.
//! Similar to server's AppModule trait.

use anyhow::Result;
use tauri::App;
use ziee_chat::{ApiRouter, Router};

/// DesktopModule trait for modular desktop features
///
/// Modules implement this trait to provide desktop-specific functionality:
/// - Backend process management
/// - System tray
/// - Auto-update
/// - Custom HTTP routes with OpenAPI documentation
///
/// All functionality (except get_server_port) communicates via HTTP routes.
pub trait DesktopModule: Send + Sync {
    /// Module name (used for logging and identification)
    fn name(&self) -> &'static str;

    /// Module version
    fn version(&self) -> &'static str {
        "1.0.0"
    }

    /// Module description
    fn description(&self) -> &'static str {
        ""
    }

    /// Initialize module with app
    ///
    /// Called during app startup. Modules can:
    /// - Access app resources (data directory, config, etc.)
    /// - Start background tasks
    /// - Initialize state
    fn init(&mut self, app: &mut App) -> Result<()>;

    /// Register API routes with OpenAPI documentation
    ///
    /// Called after init to collect routes from all modules.
    /// Routes are merged into the backend server and included in OpenAPI spec.
    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router
    }

    /// Register regular axum routes (without OpenAPI documentation)
    ///
    /// Use this for routes that don't need OpenAPI documentation,
    /// such as static file serving or proxy endpoints.
    fn register_routes(&self, router: Router) -> Router {
        router
    }

    /// Cleanup on shutdown
    ///
    /// Called when the app is shutting down. Modules should:
    /// - Stop background tasks
    /// - Save state
    /// - Release resources
    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
