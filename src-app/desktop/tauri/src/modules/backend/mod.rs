//! Backend Module
//!
//! Manages embedded backend server lifecycle

pub mod commands;
mod handlers;
mod routes;
mod state;

#[cfg(not(debug_assertions))]
mod static_files;

pub use state::BackendState;

use crate::module_api::DesktopModule;
use anyhow::Result;
use axum::{body::Body, http::Request, response::Response};
use std::sync::{Arc, OnceLock};
use tauri::{App, Manager};
use ziee::ApiRouter;

/// Global storage for backend config (set during init, used when starting server)
static BACKEND_CONFIG: OnceLock<ziee::Config> = OnceLock::new();
static BACKEND_STATE: OnceLock<BackendState> = OnceLock::new();
static JWT_SERVICE: OnceLock<Arc<ziee::JwtService>> = OnceLock::new();

/// Get the JWT service (for Tauri commands)
pub fn get_jwt_service() -> Option<&'static Arc<ziee::JwtService>> {
    JWT_SERVICE.get()
}

pub struct BackendModule {
    config_file: Option<String>,
}

impl BackendModule {
    pub fn new(config_file: Option<String>) -> Self {
        Self { config_file }
    }
}

impl DesktopModule for BackendModule {
    fn name(&self) -> &'static str {
        "backend"
    }

    fn description(&self) -> &'static str {
        "Embedded backend server lifecycle management"
    }

    fn init(&mut self, app: &mut App) -> Result<()> {
        tracing::info!("Initializing backend module...");

        // Load config from file or generate default
        let mut config = if let Some(ref config_path) = self.config_file {
            tracing::info!("Loading config from file: {}", config_path);
            ziee::Config::load_from(Some(config_path.clone()))
                .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?
        } else {
            // Get app data directory for backend configuration
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?;

            tracing::info!("App data directory: {:?}", data_dir);

            // Create config directory
            std::fs::create_dir_all(&data_dir)?;

            // Find available port for backend
            let port = ziee::find_available_port(8080, 8180)
                .ok_or_else(|| anyhow::anyhow!("No available ports in range 8080-8180"))?;

            tracing::info!("Selected port {} for backend server", port);

            // Create backend configuration
            create_desktop_config(&data_dir, port)?
        };

        // Desktop always needs permissive CORS (dynamic port, varying frontend origin)
        config.server.cors = None;

        let port = config.server.port;
        tracing::info!("Backend will use port {}", port);

        // Create backend state
        let state = BackendState::new(port);

        // Store state in app for Tauri command access
        app.manage(state.clone());

        // Store config and state globally for server start
        BACKEND_CONFIG
            .set(config)
            .map_err(|_| anyhow::anyhow!("Backend config already set"))?;
        BACKEND_STATE
            .set(state)
            .map_err(|_| anyhow::anyhow!("Backend state already set"))?;

        tracing::info!("Backend module initialized (server will start after route collection)");
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        tracing::info!("Registering backend API routes");
        router.merge(routes::backend_api_routes())
    }

    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down backend module...");

        // Cleanup backend resources
        tauri::async_runtime::block_on(async {
            ziee::cleanup_server().await;
        });

        Ok(())
    }
}

use crate::modules::auth::ensure_desktop_admin;
use crate::modules::llm_provider::AutoAssignProviderHandler;

// =====================================================
// Backend Server Startup
// =====================================================

/// Start the backend server with collected routes from all modules
///
/// This should be called from lib.rs after all modules have been initialized
/// and routes have been collected.
pub fn start_backend_server(desktop_routes: ApiRouter, app_handle: tauri::AppHandle) {
    let config = BACKEND_CONFIG
        .get()
        .expect("Backend config not initialized - call init() first")
        .clone();
    let state = BACKEND_STATE
        .get()
        .expect("Backend state not initialized - call init() first")
        .clone();

    tracing::info!("Starting backend server with desktop routes...");

    // Create desktop-specific event handlers
    let handlers: Vec<Arc<dyn ziee::EventHandler>> = vec![AutoAssignProviderHandler::new()];

    tauri::async_runtime::spawn(async move {
        match ziee::start_server_with_routes(
            config,
            |router, jwt| {
            // Store JWT service for Tauri command access
            let _ = JWT_SERVICE.set(jwt.clone());
            tracing::info!("JWT service stored for Tauri commands");

            // Initialize desktop repositories with server's pool
            // Repos is available here because start_server_with_routes
            // initializes it before calling this closure
            let pool = ziee::Repos.pool().clone();
            crate::core::init_desktop_repositories(pool);
            tracing::info!("Desktop repositories initialized with server pool");

            let router = router.merge(desktop_routes);

            // Development: proxy non-API requests to Vite dev server
            // This enables Playwright testing by serving both API and frontend from same origin
            #[cfg(debug_assertions)]
            let router = {
                tracing::info!("Development mode: enabling Vite proxy fallback");
                router.fallback(proxy_to_vite)
            };

            // Production: serve embedded static files
            #[cfg(not(debug_assertions))]
            let router = {
                tracing::info!("Production mode: serving embedded static files");
                router.fallback(static_files::serve_embedded_files)
            };

            router
            },
            handlers,
        )
        .await
        {
            Ok(addr) => {
                tracing::info!("Backend server started successfully on {}", addr);

                // Run desktop-specific migrations
                if let Err(e) = run_desktop_migrations().await {
                    tracing::error!("Failed to run desktop migrations: {}", e);
                }

                // Ensure admin exists (create on first run)
                if let Err(e) = ensure_desktop_admin().await {
                    tracing::error!("Failed to ensure desktop admin: {}", e);
                }

                state.set_ready(true);

                // Create window now that server is ready
                create_main_window(&app_handle);
            }
            Err(e) => {
                tracing::error!("Failed to start backend server: {}", e);
            }
        }
    });
}

/// Run desktop-specific database migrations
async fn run_desktop_migrations() -> Result<()> {
    tracing::info!("Running desktop migrations...");

    let pool = ziee::Repos.pool();

    // Use set_ignore_missing(true) because server migrations are tracked
    // in the same _sqlx_migrations table but are not in our migrations folder
    sqlx::migrate!("./migrations")
        .set_ignore_missing(true)
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Desktop migration failed: {}", e))?;

    tracing::info!("Desktop migrations completed successfully");
    Ok(())
}


/// Proxy handler to forward non-API requests to Vite dev server
#[cfg(debug_assertions)]
async fn proxy_to_vite(req: Request<Body>) -> Result<Response<Body>, axum::http::StatusCode> {
    let vite_url =
        std::env::var("VITE_DEV_URL").unwrap_or_else(|_| "http://localhost:1420".to_string());
    let uri = req.uri();
    let path_and_query = uri
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or(uri.path());

    let proxy_url = format!("{}{}", vite_url, path_and_query);

    match reqwest::get(&proxy_url).await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes()
                .await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

            let mut builder = Response::builder().status(status);
            for (key, value) in headers.iter() {
                builder = builder.header(key.as_str(), value);
            }
            builder
                .body(Body::from(body))
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(_) => Err(axum::http::StatusCode::BAD_GATEWAY),
    }
}

/// Create desktop-specific configuration for the backend
fn create_desktop_config(
    data_dir: &std::path::Path,
    port: u16,
) -> Result<ziee::Config> {
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
    let config: ziee::Config = serde_yaml::from_str(&yaml_str)?;

    Ok(config)
}

/// Create the main window with platform-specific customizations
fn create_main_window(app_handle: &tauri::AppHandle) {
    tracing::info!("Creating main window...");

    // macOS/Linux: no decorations initially
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let mut main_window_builder = tauri::webview::WebviewWindowBuilder::new(
        app_handle,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("")
    .inner_size(1200.0, 800.0)
    .min_inner_size(400.0, 600.0)
    .resizable(true)
    .fullscreen(false)
    .decorations(false)
    .center()
    .effects(tauri::utils::config::WindowEffectsConfig {
        effects: vec![
            tauri::window::Effect::Mica,
            tauri::window::Effect::Acrylic,
            tauri::window::Effect::Blur,
        ],
        state: Some(tauri::window::EffectState::Active),
        radius: Some(8.0),
        color: None,
    });

    // Windows: has decorations
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let main_window_builder = tauri::webview::WebviewWindowBuilder::new(
        app_handle,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("")
    .inner_size(1200.0, 800.0)
    .min_inner_size(800.0, 600.0)
    .resizable(true)
    .fullscreen(false)
    .decorations(true)
    .center()
    .effects(tauri::utils::config::WindowEffectsConfig {
        effects: vec![
            tauri::window::Effect::Mica,
            tauri::window::Effect::Acrylic,
            tauri::window::Effect::Blur,
        ],
        state: Some(tauri::window::EffectState::Active),
        radius: Some(8.0),
        color: None,
    });

    // macOS: overlay titlebar with native traffic light position (no glitch on resize)
    #[cfg(target_os = "macos")]
    {
        main_window_builder = main_window_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .decorations(true)
            .traffic_light_position(tauri::LogicalPosition::new(12.0, 22.0));
    }

    // Linux: transparent
    #[cfg(target_os = "linux")]
    {
        main_window_builder = main_window_builder.transparent(true);
    }

    main_window_builder.build().unwrap();

    // Post-build: Windows overlay
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        use tauri_plugin_decorum::WebviewWindowExt;
        let main_window = app_handle.get_webview_window("main").unwrap();
        main_window.create_overlay_titlebar().unwrap();
    }

    tracing::info!("Main window created successfully");
}
