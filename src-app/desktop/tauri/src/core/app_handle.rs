//! Global AppHandle Storage
//!
//! Provides global access to Tauri's AppHandle for axum route handlers.
//! This is necessary because route handlers run in the axum context but need
//! access to Tauri APIs (like the updater).

use std::sync::OnceLock;
use tauri::AppHandle;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Store the AppHandle globally during app setup
///
/// Called once during Tauri setup. Panics if called more than once.
pub fn set_app_handle(handle: AppHandle) {
    APP_HANDLE
        .set(handle)
        .expect("AppHandle already set - this should only be called once during setup");
}

/// Get a reference to the global AppHandle
///
/// Used by axum route handlers to access Tauri APIs.
/// Panics if called before set_app_handle.
pub fn get_app_handle() -> &'static AppHandle {
    APP_HANDLE
        .get()
        .expect("AppHandle not initialized - ensure set_app_handle is called during setup")
}

/// Check if AppHandle has been initialized
pub fn is_app_handle_set() -> bool {
    APP_HANDLE.get().is_some()
}
