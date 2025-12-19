//! Ziee Chat Desktop - Library
//!
//! Tauri application with modular desktop features.
//! All functionality (except get_server_port) communicates via HTTP routes.

mod core;
mod module_api;
pub mod modules;
pub mod openapi;

use anyhow::Result;

/// Run the desktop application
///
/// # Arguments
/// * `config_file` - Optional path to a YAML config file (like server's dev.yaml)
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(config_file: Option<String>) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Ziee Chat Desktop...");
    if let Some(ref path) = config_file {
        tracing::info!("Using config file: {}", path);
    }

    // Create desktop modules with config
    let mut modules = core::create_desktop_modules(config_file);
    tracing::info!("Created {} desktop modules", modules.len());

    tauri::Builder::default()
        .plugin(tauri_plugin_decorum::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            tracing::info!("Tauri setup starting...");

            // Store AppHandle globally for route handlers
            core::set_app_handle(app.handle().clone());

            // Initialize all modules
            core::initialize_modules(&mut modules, app)?;

            // Collect API routes from all modules (with OpenAPI documentation)
            let desktop_routes = core::build_desktop_api_routes(&modules);

            // Start the backend server with collected routes (pass AppHandle for window creation)
            modules::backend::start_backend_server(desktop_routes, app.handle().clone());

            tracing::info!("Tauri setup complete");
            Ok(())
        })
        // Tauri commands (desktop-only functionality)
        .invoke_handler(tauri::generate_handler![
            crate::modules::backend::commands::get_server_port,
            crate::modules::auth::commands::auto_login,
        ])
        // Window event handler for cleanup
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    tracing::info!("Main window close requested, cleaning up...");
                    tauri::async_runtime::spawn(async move {
                        ziee_chat::cleanup_server().await;
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
