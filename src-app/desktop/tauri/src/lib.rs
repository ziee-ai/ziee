/**
 * Ziee Chat Desktop - Library
 *
 * Tauri application with modular desktop features
 */

mod core;
mod module_api;
mod modules;

use anyhow::Result;

/// Run the desktop application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Ziee Chat Desktop...");

    // Create desktop modules
    let mut modules = core::create_desktop_modules();
    tracing::info!("Created {} desktop modules", modules.len());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            tracing::info!("Tauri setup starting...");

            // Initialize all modules
            core::initialize_modules(&mut modules, app)?;

            tracing::info!("Tauri setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Backend commands
            crate::modules::backend::commands::get_server_port,
            crate::modules::backend::commands::get_backend_status,
            crate::modules::backend::commands::restart_backend,
            // Window commands
            crate::modules::window::commands::minimize_window,
            crate::modules::window::commands::maximize_window,
            crate::modules::window::commands::unmaximize_window,
            crate::modules::window::commands::close_window,
            crate::modules::window::commands::toggle_fullscreen,
            crate::modules::window::commands::is_window_maximized,
            // File dialog commands
            crate::modules::file_dialog::commands::open_file_dialog,
            crate::modules::file_dialog::commands::open_folder_dialog,
            crate::modules::file_dialog::commands::save_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
