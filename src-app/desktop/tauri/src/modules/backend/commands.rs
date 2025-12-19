//! Backend Tauri Commands
//!
//! Only essential Tauri commands that can't be HTTP routes.
//! All other functionality is exposed via HTTP routes.

use super::state::BackendState;
use tauri::State;

/// Get the backend server port
///
/// This is the only Tauri command needed - the frontend needs to know
/// the port to connect to before making HTTP requests.
#[tauri::command]
pub async fn get_server_port(state: State<'_, BackendState>) -> Result<u16, String> {
    Ok(state.get_port())
}
