//! Desktop Modules
//!
//! Feature modules:
//! - auth: Desktop authentication and user management
//! - backend: Backend server lifecycle and status routes
//! - settings: Desktop-specific settings management
//! - tray: System tray integration
//! - updater: Auto-update via HTTP routes

pub mod auth;
pub mod backend;
pub mod settings;
pub mod tray;
pub mod updater;
