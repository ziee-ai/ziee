use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use axum::http::header::HeaderName;
use axum::http::Method;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::core::config::Config;
use crate::core::EventBus;
use crate::module_api::{AppModule, ModuleContext};
use crate::modules::{AppModule as AppMod, AuthModule, HealthModule, user::UserModule, hardware::HardwareModule, llm_provider::LlmProviderModule, llm_repository::LlmRepositoryModule, llm_model::LlmModelModule, assistant::AssistantModule, mcp::McpModule, hub::HubModule, chat::ChatModule};

/// Create and initialize all application modules
pub fn create_modules() -> Vec<Box<dyn AppModule>> {
    vec![
        Box::new(HealthModule::new()),
        Box::new(AppMod::new()),
        Box::new(AuthModule::new()),
        Box::new(UserModule::new()),
        Box::new(HardwareModule::new()),
        Box::new(LlmProviderModule::new()),
        Box::new(LlmRepositoryModule::new()),
        Box::new(LlmModelModule::new()),
        Box::new(AssistantModule::new()),
        Box::new(McpModule::new()),
        Box::new(HubModule::new()),
        Box::new(ChatModule::new()),
    ]
}

/// Initialize all modules with the given context
pub fn initialize_modules(
    modules: &mut [Box<dyn AppModule>],
    context: &ModuleContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for module in modules.iter_mut() {
        module.init(context).map_err(|e| {
            format!("Failed to initialize module {}: {}", module.name(), e)
        })?;
        tracing::info!("Initialized module: {}", module.name());
    }
    Ok(())
}

/// Register event handlers from all modules
pub fn register_event_handlers(
    modules: &[Box<dyn AppModule>],
    pool: Arc<PgPool>,
) -> EventBus {
    let mut event_bus = EventBus::new(pool);

    for module in modules.iter() {
        for handler in module.register_event_handlers() {
            tracing::info!(
                "Registering event handler '{}' for module: {}",
                handler.handler_name(),
                module.name()
            );
            event_bus.register(handler);
        }
    }

    tracing::info!("Registered {} event handlers total", event_bus.handler_count());
    event_bus
}

/// Build API router with all module routes
pub fn build_api_router(
    modules: &[Box<dyn AppModule>],
    api_prefix: &str,
    _pool: PgPool,
) -> (ApiRouter, OpenApi) {
    // Build combined router from all modules
    // Modules handle their own state requirements internally
    let mut combined_router = ApiRouter::new();
    for module in modules.iter() {
        combined_router = module.register_routes(combined_router);
    }

    // Create OpenAPI documentation
    let api_doc = OpenApi::default();

    // Nest all routes under the api_prefix
    let api_router = ApiRouter::new().nest(api_prefix, combined_router);

    (api_router, api_doc)
}

/// Create CORS layer from configuration
pub fn create_cors_layer(config: &Config) -> CorsLayer {
    if let Some(ref cors_config) = config.server.cors {
        let origins: Vec<_> = cors_config
            .allow_origins
            .iter()
            .filter_map(|origin| {
                if origin == "*" {
                    None
                } else {
                    origin.parse::<axum::http::HeaderValue>().ok()
                }
            })
            .collect();

        let methods: Vec<Method> = cors_config
            .allow_methods
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect();

        let headers: Vec<HeaderName> = cors_config
            .allow_headers
            .iter()
            .filter_map(|h| {
                if h == "*" {
                    None
                } else {
                    h.parse().ok()
                }
            })
            .collect();

        let mut layer = CorsLayer::new();

        // Set origins
        if cors_config.allow_origins.contains(&"*".to_string()) || origins.is_empty() {
            layer = layer.allow_origin(Any);
        } else {
            layer = layer.allow_origin(AllowOrigin::list(origins));
        }

        // Set methods
        if methods.is_empty() {
            layer = layer.allow_methods(Any);
        } else {
            layer = layer.allow_methods(methods);
        }

        // Set headers
        if cors_config.allow_headers.contains(&"*".to_string()) || headers.is_empty() {
            layer = layer.allow_headers(Any);
        } else {
            layer = layer.allow_headers(headers);
        }

        layer
    } else {
        // Default permissive CORS if not configured
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}

