//! Window Management Commands

use tauri::Window;

#[tauri::command]
pub async fn minimize_window(window: Window) -> Result<(), String> {
    window
        .minimize()
        .map_err(|e| format!("Failed to minimize window: {}", e))
}

#[tauri::command]
pub async fn maximize_window(window: Window) -> Result<(), String> {
    window
        .maximize()
        .map_err(|e| format!("Failed to maximize window: {}", e))
}

#[tauri::command]
pub async fn unmaximize_window(window: Window) -> Result<(), String> {
    window
        .unmaximize()
        .map_err(|e| format!("Failed to unmaximize window: {}", e))
}

#[tauri::command]
pub async fn close_window(window: Window) -> Result<(), String> {
    window
        .close()
        .map_err(|e| format!("Failed to close window: {}", e))
}

#[tauri::command]
pub async fn toggle_fullscreen(window: Window) -> Result<(), String> {
    let is_fullscreen = window
        .is_fullscreen()
        .map_err(|e| format!("Failed to get fullscreen state: {}", e))?;

    window
        .set_fullscreen(!is_fullscreen)
        .map_err(|e| format!("Failed to toggle fullscreen: {}", e))
}

#[tauri::command]
pub async fn is_window_maximized(window: Window) -> Result<bool, String> {
    window
        .is_maximized()
        .map_err(|e| format!("Failed to get maximized state: {}", e))
}
