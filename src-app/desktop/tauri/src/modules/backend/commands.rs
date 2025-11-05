//! Backend Tauri Commands
//!
//! Commands callable from the frontend

use super::state::{BackendState, BackendStatus};
use tauri::State;

/// Get the backend server port
#[tauri::command]
pub async fn get_server_port(state: State<'_, BackendState>) -> Result<u16, String> {
    Ok(state.get_port())
}

/// Get backend server status
#[tauri::command]
pub async fn get_backend_status(state: State<'_, BackendState>) -> Result<BackendStatus, String> {
    let port = state.get_port();
    let ready = state.is_ready();

    Ok(BackendStatus {
        running: true, // Server is always running in embedded mode
        ready,
        port,
    })
}

/// Restart backend server
/// Note: In embedded mode, restarting requires restarting the entire app
#[tauri::command]
pub async fn restart_backend(_state: State<'_, BackendState>) -> Result<(), String> {
    Err("Backend restart not supported in embedded mode. Please restart the application.".to_string())
}
