//! Backend Module
//!
//! Manages embedded backend server lifecycle

pub mod commands;
mod state;

pub use state::BackendState;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::{App, Manager};

pub struct BackendModule {
    state: Option<BackendState>,
}

impl BackendModule {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl DesktopModule for BackendModule {
    fn name(&self) -> &'static str {
        "backend"
    }

    fn init(&mut self, app: &mut App) -> Result<()> {
        tracing::info!("Initializing backend module...");

        // Get app data directory for backend configuration
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?;

        tracing::info!("App data directory: {:?}", data_dir);

        // Create config directory
        std::fs::create_dir_all(&data_dir)?;

        // Find available port for backend
        let port = ziee_chat::find_available_port(8080, 8180)
            .ok_or_else(|| anyhow::anyhow!("No available ports in range 8080-8180"))?;

        tracing::info!("Selected port {} for backend server", port);

        // Create backend configuration
        let config = create_desktop_config(&data_dir, port)?;

        // Create backend state
        let state = BackendState::new(port);

        // Store state in app for command access
        app.manage(state.clone());

        // Start backend server in background
        let state_clone = state.clone();
        tauri::async_runtime::spawn(async move {
            match ziee_chat::start_server(config).await {
                Ok(addr) => {
                    tracing::info!("Backend server started successfully on {}", addr);
                    state_clone.set_ready(true);
                }
                Err(e) => {
                    tracing::error!("Failed to start backend server: {}", e);
                }
            }
        });

        self.state = Some(state);

        tracing::info!("Backend module initialized");
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down backend module...");

        // Cleanup backend resources
        tauri::async_runtime::block_on(async {
            ziee_chat::cleanup_server().await;
        });

        Ok(())
    }
}

/// Create desktop-specific configuration for the backend
fn create_desktop_config(
    data_dir: &std::path::Path,
    port: u16,
) -> Result<ziee_chat::Config> {
    use serde_yaml;
    use std::collections::HashMap;

    // Create config structure
    let mut config_map = HashMap::new();

    // Server configuration
    let mut server_config = HashMap::new();
    server_config.insert("host".to_string(), "127.0.0.1".to_string());
    server_config.insert("port".to_string(), port.to_string());
    server_config.insert("api_prefix".to_string(), "/api".to_string());
    config_map.insert("server".to_string(), server_config);

    // App configuration
    let mut app_config = HashMap::new();
    app_config.insert(
        "data_dir".to_string(),
        data_dir.to_string_lossy().to_string(),
    );
    config_map.insert("app".to_string(), app_config);

    // Database configuration (embedded PostgreSQL)
    let mut db_config = HashMap::new();
    db_config.insert("embedded".to_string(), true.to_string());
    let db_path = data_dir.join("database");
    db_config.insert("path".to_string(), db_path.to_string_lossy().to_string());
    config_map.insert("database".to_string(), db_config);

    // JWT configuration
    let mut jwt_config = HashMap::new();
    // Generate random secret for desktop app
    use rand::Rng;
    let secret: String = rand::rng()
        .random_iter::<u8>()
        .take(32)
        .map(|b| format!("{:02x}", b))
        .collect();
    jwt_config.insert("secret".to_string(), secret);
    jwt_config.insert("access_token_expiry".to_string(), "3600".to_string());
    jwt_config.insert("refresh_token_expiry".to_string(), "604800".to_string());
    config_map.insert("jwt".to_string(), jwt_config);

    // Logging configuration
    let mut logging_config = HashMap::new();
    logging_config.insert("level".to_string(), "info".to_string());
    logging_config.insert("format".to_string(), "compact".to_string());
    config_map.insert("logging".to_string(), logging_config);

    // Parse config from YAML
    let yaml_str = serde_yaml::to_string(&config_map)?;
    let config: ziee_chat::Config = serde_yaml::from_str(&yaml_str)?;

    Ok(config)
}
