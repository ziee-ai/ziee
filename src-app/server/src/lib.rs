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
// Re-exported so integration tests (which construct repositories directly
// against the test DB pool) can initialise the same at-rest storage_key
// that the spawned server process used. Without this, repo.get() in the
// test process can't decrypt rows the server wrote, and resolve-fallback
// returns None. See common::secret::resolve_optional_secret.
#[doc(hidden)]
pub use core::secrets::{init_storage_key, storage_key};
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
pub use modules::mcp::client::auth::{
    OAuthClientConfig, StoredToken, refresh_token as oauth_refresh_token,
};
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

// Re-export memory chat-extension functions for integration tests
// (tier 5 real-LLM tests need to invoke the extraction + summarizer
// pipelines directly).
#[doc(hidden)]
pub mod memory_extensions {
    pub use crate::modules::chat::extensions::memory::extractor;
    pub use crate::modules::chat::extensions::memory::summarizer;
}

// Re-export code_sandbox surface for integration tests (tier 2 + 3).
#[doc(hidden)]
pub mod code_sandbox {
    pub use crate::modules::code_sandbox::{
        code_sandbox_server_id, loopback_host, CodeSandboxRepository,
        SANDBOX_KNOWN_REVISIONS_TOML, SANDBOX_ROOTFS_SCHEMA_VERSION,
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

/// Initialize tracing based on config. Closes 14-core F-23 (Info):
/// uses EnvFilter so operators can do `RUST_LOG=ziee_chat=debug,sqlx=warn`
/// for module-level filtering. Falls back to the config-file level when
/// RUST_LOG is unset.
fn init_tracing(config: &Config) {
    use tracing_subscriber::filter::EnvFilter;
    let config_level = config
        .logging
        .as_ref()
        .map(|l| l.level.as_str())
        .unwrap_or("info");
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config_level));

    let format = config
        .logging
        .as_ref()
        .map(|l| l.format.as_str())
        .unwrap_or("default");
    match format {
        "compact" => {
            tracing_subscriber::fmt()
                .compact()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
        "pretty" => {
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .try_init()
                .ok();
        }
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

    // Initialize at-rest secret storage key — see core::secrets.
    core::secrets::init_storage_key(
        config
            .secrets
            .as_ref()
            .and_then(|s| s.storage_key.clone()),
    );

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

    // Set up JWT service. try_new refuses weak/placeholder secrets so
    // the server never boots with a known signer. Closes 01-auth F-10
    // + 14-core F-03.
    let jwt_service = Arc::new(
        modules::auth::JwtService::try_new(config.jwt.clone())
            .map_err(|e| {
                tracing::error!("Failed to initialize JWT service: {}", e);
                e
            })?,
    );
    tracing::info!("JWT service initialized");

    // Build API router with all module routes
    let (api_router, mut api_doc) = core::app_builder::build_api_router(
        &modules,
        &config.server.api_prefix,
        (*module_context.db_pool).clone(),
    );

    // Convert ApiRouter to Router and add layers.
    //
    // SECURITY: matches the middleware stack in main.rs::main —
    // 16 MB body limit, 60s timeout, security headers, CORS.
    // Closes 14-core F-01 + 05-file F-09 generalization + A3 headers.
    // Rate limiter — see main.rs for rationale. 5 req/s per peer IP, burst 60.
    let (rl_per_sec, rl_burst) = config
        .server
        .rate_limit
        .as_ref()
        .map(|r| (r.per_second, r.burst_size))
        .unwrap_or((5, 60));
    let governor_conf = std::sync::Arc::new(
        tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(rl_per_sec)
            .burst_size(rl_burst)
            .key_extractor(tower_governor::key_extractor::PeerIpKeyExtractor)
            .finish()
            .expect("Failed to build governor config"),
    );
    let governor_layer = tower_governor::GovernorLayer {
        config: governor_conf,
    };

    let app = api_router
        .finish_api(&mut api_doc)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60)))
        .layer(governor_layer)
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            axum::http::HeaderValue::from_static("no-referrer"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("strict-transport-security"),
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
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
        // into_make_service_with_connect_info surfaces the TCP peer
        // address for tower_governor's PeerIpKeyExtractor — same fix
        // as main.rs. Without it, rate-limited requests return 500.
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
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
