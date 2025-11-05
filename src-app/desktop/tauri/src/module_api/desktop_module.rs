//! Desktop Module API
//!
//! Trait-based module system for Tauri desktop features.
//! Similar to server's AppModule trait.

use anyhow::Result;
use tauri::App;

/// DesktopModule trait for modular desktop features
///
/// Modules implement this trait to provide desktop-specific functionality:
/// - Backend process management
/// - Window management
/// - System tray
/// - File dialogs
/// - Auto-update
/// - etc.
pub trait DesktopModule: Send + Sync {
    /// Module name (used for logging and identification)
    fn name(&self) -> &'static str;

    /// Initialize module with app
    ///
    /// Called during app startup. Modules can:
    /// - Access app resources (data directory, config, etc.)
    /// - Start background tasks
    /// - Initialize state
    fn init(&mut self, app: &mut App) -> Result<()>;

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
