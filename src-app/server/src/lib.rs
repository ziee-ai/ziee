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

// Re-export types for desktop/external use
pub use core::config::Config;
pub use core::{Repos, EventBus, EventHandler, AppEvent};
pub use module_api::ModuleContext as ServerContext;
pub use modules::auth::{JwtService, AuthResponse, hash_password};
pub use modules::user::models::User;
pub use modules::llm_provider::events::LlmProviderEvent;
pub use modules::llm_provider::UserKeyRepository;
pub use modules::chat::core::ai_provider::resolve_api_key_for_user;
pub use common::AppError;
// Re-export async_trait for consistent EventHandler implementations
pub use async_trait::async_trait;

// Re-export MCP client types for integration tests
pub use modules::mcp::client::http::HttpMcpClient;
pub use modules::mcp::client::traits::McpClient;
pub use modules::mcp::{McpServer, TransportType, UsageMode};
pub use modules::mcp::sampling::handler::{ChatSamplingHandler, SamplingHandler};
pub use modules::mcp::sampling::models::{
    SamplingContent, SamplingCreateMessageRequest, SamplingCreateMessageResult,
};
pub use ai_providers::Provider as AiProvider;

// Re-export elicitation primitives for integration tests.
#[doc(hidden)]
pub use modules::mcp::elicitation::models::{
    ElicitationResponse, ElicitationStartedNotification,
};
#[doc(hidden)]
pub use modules::mcp::elicitation::registry as elicitation_registry;

// Re-export MCP content types for integration tests
#[doc(hidden)]
pub use modules::chat::extensions::mcp::content::{McpContentData, RichFile};

// Re-export code_sandbox surface for integration tests (tier 2 + 3).
#[doc(hidden)]
pub mod code_sandbox {
    pub use crate::modules::code_sandbox::{
        code_sandbox_server_id, loopback_host, CodeSandboxRepository,
    };
}
// MCP repository for integration tests that need McpRepository::list_accessible.
#[doc(hidden)]
pub mod mcp {
    pub use crate::modules::mcp::McpRepository;
}

// Re-export axum types for route building
pub use axum::{Extension, Json, extract::State, http::StatusCode};
pub use axum::routing::{get, post};
pub use axum::Router;

// Re-export aide types for route building with OpenAPI
pub use aide::axum::ApiRouter;
pub use aide::axum::routing::{get_with, post_with, put_with, delete_with};
pub use aide::transform::TransformOperation;

// Re-export app_builder functions for desktop OpenAPI generation
pub use core::app_builder::{create_modules, build_api_router, initialize_modules};
pub use core::database::initialize_database;
pub use core::init_repositories;
pub use module_api::AppModule;

/// Server setup result containing components needed for customization
struct ServerSetup {
    app: Router,
    jwt_service: Arc<JwtService>,
    addr: String,
}

/// Initialize tracing based on config
fn init_tracing(config: &Config) {
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
}

/// Initialize app data directory
fn init_data_dir(config: &Config) {
    if let Some(ref app_config) = config.app {
        let data_dir = std::path::PathBuf::from(&app_config.data_dir);
        core::set_app_data_dir(data_dir);
    } else {
        let default_data_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ziee-chat");
        core::set_app_data_dir(default_data_dir);
    }
}

/// Common server setup - initializes all components and returns the router
async fn setup_server(
    config: Config,
    additional_handlers: Vec<Arc<dyn core::EventHandler>>,
) -> Result<ServerSetup, Box<dyn std::error::Error + Send + Sync>> {
    // Initialize database
    let pool = core::database::initialize_database(&config).await?;
    tracing::info!("Database initialized with {} connections", pool.num_idle());

    // Initialize global repository factory
    core::init_repositories((*pool).clone());
    tracing::info!("Global repository factory initialized");

    // Initialize modules
    let module_context = ModuleContext::new(pool.clone(), Arc::new(config.clone()));
    let mut modules = core::app_builder::create_modules();

    // Initialize all modules
    core::app_builder::initialize_modules(&mut modules, &module_context)?;

    // Register event handlers from all modules
    let mut event_bus = core::app_builder::register_event_handlers(
        &modules,
        pool.clone(),
    );

    // Register additional handlers (e.g., from desktop app)
    for handler in additional_handlers {
        tracing::info!(
            "Registering additional event handler: {}",
            handler.handler_name()
        );
        event_bus.register(handler);
    }

    let event_bus = Arc::new(event_bus);
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
        .layer(axum::Extension(jwt_service.clone()))
        .layer(cors);

    let addr = config.server_address();

    Ok(ServerSetup {
        app,
        jwt_service,
        addr,
    })
}

/// Start the server with the given router
async fn run_server(
    app: Router,
    addr: String,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;

    tracing::info!(
        "Ziee Chat backend server started successfully on {}",
        local_addr
    );

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("Failed to run server");
    });

    Ok(local_addr)
}

/// Initialize and start the backend server
pub async fn start_server(
    config: Config,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    init_tracing(&config);
    tracing::info!("Starting Ziee Chat backend server");
    init_data_dir(&config);

    let setup = setup_server(config, vec![]).await?;
    run_server(setup.app, setup.addr).await
}

/// Initialize and start the backend server with custom routes and event handlers
/// Allows desktop/external apps to add custom endpoints and event handlers
pub async fn start_server_with_routes<F>(
    config: Config,
    route_builder: F,
    additional_handlers: Vec<Arc<dyn core::EventHandler>>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(Router, Arc<JwtService>) -> Router,
{
    init_tracing(&config);
    tracing::info!("Starting Ziee Chat backend server (with custom routes)");
    init_data_dir(&config);

    let setup = setup_server(config, additional_handlers).await?;
    let app = route_builder(setup.app, setup.jwt_service);
    run_server(app, setup.addr).await
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
