use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use axum::http::Method;
use axum::http::header::HeaderName;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::core::EventBus;
use crate::core::config::Config;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext};

/// Create and initialize all application modules
///
/// Modules are automatically discovered at link time using linkme distributed slices.
/// Each module registers itself using #[distributed_slice(MODULE_ENTRIES)].
pub fn create_modules() -> Vec<Box<dyn AppModule>> {
    // Collect modules from distributed slice
    let mut entries: Vec<_> = MODULE_ENTRIES.iter().collect();

    // Sort by order (lower numbers first)
    entries.sort_by_key(|e| e.order);

    // Instantiate modules using their constructors
    let modules: Vec<Box<dyn AppModule>> =
        entries.iter().map(|entry| (entry.constructor)()).collect();

    tracing::info!("Loaded {} modules in order:", modules.len());
    for entry in entries.iter() {
        tracing::debug!(
            "  - {} (order: {}) - {}",
            entry.name,
            entry.order,
            entry.description
        );
    }

    modules
}

/// Initialize all modules with the given context
pub fn initialize_modules(
    modules: &mut [Box<dyn AppModule>],
    context: &ModuleContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for module in modules.iter_mut() {
        module
            .init(context)
            .map_err(|e| format!("Failed to initialize module {}: {}", module.name(), e))?;
        tracing::info!("Initialized module: {}", module.name());
    }
    Ok(())
}

/// Register event handlers from all modules
pub fn register_event_handlers(modules: &[Box<dyn AppModule>], pool: Arc<PgPool>) -> EventBus {
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

    tracing::info!(
        "Registered {} event handlers total",
        event_bus.handler_count()
    );
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

    // Create OpenAPI documentation. Closes 14-core F-24 (Info): adds
    // a `bearerAuth` security scheme so generated clients (and the
    // Redoc/Swagger UI rendering of the spec) know to send the JWT
    // as `Authorization: Bearer …`. Per-operation `security` arrays
    // are still up to individual handlers (most use `with_permission`
    // which already encodes the permission requirement).
    let mut api_doc = OpenApi::default();
    let mut components = api_doc.components.unwrap_or_default();
    components.security_schemes.insert(
        "bearerAuth".to_string(),
        aide::openapi::ReferenceOr::Item(aide::openapi::SecurityScheme::Http {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
            description: Some(
                "JWT obtained from POST /auth/login or POST /auth/register, \
                 sent as `Authorization: Bearer <token>`."
                    .to_string(),
            ),
            extensions: Default::default(),
        }),
    );
    api_doc.components = Some(components);

    // Nest all routes under the api_prefix
    let api_router = ApiRouter::new().nest(api_prefix, combined_router);

    (api_router, api_doc)
}

/// Conditionally apply the global rate limiter (tower-governor).
///
/// Returns the router unchanged when `server.rate_limit.enabled == false`,
/// so the `GovernorLayer` is never installed in that case. Called from BOTH
/// `lib.rs::build_server` and `main.rs::main` so the two stay in sync.
///
/// Why the toggle exists: the built-in code_sandbox + memory MCP servers are
/// reached over loopback (`http://127.0.0.1`), so every internal tool-call
/// request shares the same `PeerIpKeyExtractor` bucket as real user traffic.
/// A rapid agent tool loop drains that bucket and the server starts returning
/// HTTP 429 to itself. Trusted / non-public deployments can set
/// `enabled: false` to opt out entirely. When enabled (default), the limiter
/// applies to all traffic as before (default 5 req/s, burst 60).
pub fn apply_rate_limit_layer(router: axum::Router, config: &Config) -> axum::Router {
    let (enabled, per_second, burst_size) = config
        .server
        .rate_limit
        .as_ref()
        .map(|r| (r.enabled, r.per_second, r.burst_size))
        .unwrap_or((true, 5, 60));

    if !enabled {
        tracing::warn!(
            "Rate limiting DISABLED via config (server.rate_limit.enabled=false) — \
             no per-IP throttling is applied to any route. Safe only for trusted / \
             non-public deployments."
        );
        return router;
    }

    let governor_conf = Arc::new(
        tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(per_second)
            .burst_size(burst_size)
            .key_extractor(tower_governor::key_extractor::PeerIpKeyExtractor)
            .finish()
            .expect("Failed to build governor config"),
    );
    router.layer(tower_governor::GovernorLayer {
        config: governor_conf,
    })
}

/// Create CORS layer from configuration.
///
/// Closes 14-core F-04 (High) at the level of "operator visibility":
/// any deployment booting with `Any/Any/Any` (either via wildcard
/// `*` in allow_origins, missing config, or empty list) gets a loud
/// `tracing::error!` at boot. Production deployments behind a
/// reverse proxy must set an explicit origin allowlist. We don't
/// hard-fail boot because dev/test environments legitimately need
/// permissive CORS; the loud log is enough to catch the misconfig
/// in `journalctl`/`docker logs` review.
pub fn create_cors_layer(config: &Config) -> CorsLayer {
    let permissive_warning = |reason: &str| {
        tracing::error!(
            "SECURITY: CORS is permissive ({}). Any origin can call \
             the API and read non-credentialed responses. Set \
             server.cors.allow_origins to an explicit allowlist for \
             production deployments (see config/prod.example.yaml). \
             Closes 14-core F-04.",
            reason
        );
    };

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
            .filter_map(|h| if h == "*" { None } else { h.parse().ok() })
            .collect();

        let mut layer = CorsLayer::new();

        // Set origins
        if cors_config.allow_origins.contains(&"*".to_string()) || origins.is_empty() {
            permissive_warning(
                "allow_origins is empty or contains '*'",
            );
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
        permissive_warning("no server.cors block in config");
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}
