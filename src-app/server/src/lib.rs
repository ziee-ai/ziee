// Core modules for modular architecture
mod common;
mod core;
mod module_api;
mod modules;
mod openapi;
mod utils;

use module_api::ModuleContext;
use std::net::SocketAddr;
use std::sync::Arc;

pub use core::config::Config;

/// Initialize and start the backend server
/// Returns the server address that was bound
pub async fn start_server(
    config: Config,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing for logging based on config
    if let Some(ref logging_config) = config.logging {
        let level = logging_config
            .level
            .parse::<tracing_subscriber::filter::LevelFilter>()
            .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO);

        match logging_config.format.as_str() {
            "compact" => {
                tracing_subscriber::fmt()
                    .compact()
                    .with_max_level(level)
                    .try_init()
                    .ok();
            }
            "pretty" => {
                tracing_subscriber::fmt()
                    .pretty()
                    .with_max_level(level)
                    .try_init()
                    .ok();
            }
            _ => {
                tracing_subscriber::fmt()
                    .with_max_level(level)
                    .try_init()
                    .ok();
            }
        }
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing_subscriber::filter::LevelFilter::INFO)
            .try_init()
            .ok();
    }

    tracing::info!("Starting Ziee Chat backend server");

    // Initialize application data directory from config
    if let Some(ref app_config) = config.app {
        let data_dir = std::path::PathBuf::from(&app_config.data_dir);
        core::set_app_data_dir(data_dir);
    } else {
        let default_data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ziee-chat");
        core::set_app_data_dir(default_data_dir);
    }

    // Initialize database
    let pool = core::database::initialize_database(&config).await?;
    tracing::info!("Database initialized with {} connections", pool.num_idle());

    // Initialize global repository factory
    core::init_repositories((*pool).clone());
    tracing::info!("Global repository factory initialized");

    // Initialize modules
    let module_context = ModuleContext::new(pool.clone());
    let mut modules = core::app_builder::create_modules();

    // Initialize all modules
    core::app_builder::initialize_modules(&mut modules, &module_context)?;

    // Register event handlers from all modules
    let event_bus = Arc::new(core::app_builder::register_event_handlers(
        &modules,
        pool.clone(),
    ));
    tracing::info!(
        "Event bus initialized with {} handlers",
        event_bus.handler_count()
    );

    // Setup CORS from config
    let cors = core::app_builder::create_cors_layer(&config);

    // Set up JWT service
    let jwt_service = Arc::new(modules::auth::JwtService::new(config.jwt.clone()));
    tracing::info!("JWT service initialized");

    // Build API router with all module routes
    let (api_router, mut api_doc) = core::app_builder::build_api_router(
        &modules,
        &config.server.api_prefix,
        (*module_context.db_pool).clone(),
    );

    // Convert ApiRouter to Router and add layers
    let app = api_router
        .finish_api(&mut api_doc)
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(axum::Extension(event_bus))
        .layer(axum::Extension(jwt_service))
        .layer(cors);

    // Get server address
    let addr = config.server_address();
    tracing::info!("Starting HTTP server on {}", addr);

    // Create listener
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;

    tracing::info!(
        "Ziee Chat backend server started successfully on {}",
        local_addr
    );

    // Spawn server in background task
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("Failed to run server");
    });

    Ok(local_addr)
}

/// Find an available port in the given range
pub fn find_available_port(start: u16, end: u16) -> Option<u16> {
    (start..=end).find(|&port| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok())
}

/// Cleanup server resources
pub async fn cleanup_server() {
    tracing::info!("Cleaning up server resources...");
    core::database::cleanup_database().await;
}

/// Generate OpenAPI specification
pub async fn generate_openapi(
    output_dir: &str,
    config_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    openapi::generate_openapi_spec(output_dir, config_file).await?;
    Ok(())
}
