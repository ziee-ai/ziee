//! OpenAPI Generation
//!
//! Generates combined OpenAPI specification including both server and desktop endpoints.

use crate::core;
use std::path::Path;

/// Generate combined OpenAPI specification (server + desktop endpoints)
///
/// This function initializes the server, builds both server and desktop routes,
/// and generates a combined OpenAPI spec that includes all endpoints.
pub async fn generate_openapi_spec(
    output_dir: &str,
    config_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Generating combined OpenAPI specification (server + desktop)...");

    // Load configuration
    let config = ziee::Config::load_from(config_file)?;

    // Initialize globals normally set by `setup_server`. Modules read
    // these during `initialize_modules` below (e.g.,
    // llm_local_runtime/runtime_version/handlers.rs unwraps
    // `get_caches_config().llm_engines_dir()`); skipping them panics
    // with "llm_engines_dir filled by Config::resolve_paths" mid-init.
    if let Some(app) = &config.app {
        ziee::set_app_data_dir(std::path::PathBuf::from(&app.data_dir));
    }
    ziee::set_caches_config(config.caches.clone());

    // OpenAPI generation only walks the router structure — it never
    // executes handlers — so we use a LAZY pool that never opens a
    // socket. The previous implementation booted embedded PostgreSQL
    // (10+ seconds + filesystem-format quirks like "Operation not
    // supported" on non-APFS volumes), wasting time and turning the
    // gen step into a flake-prone build dependency.
    use sqlx::postgres::PgPoolOptions;
    let pool = std::sync::Arc::new(
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&config.database_url())?,
    );

    // Initialize global repository factory
    ziee::init_repositories((*pool).clone());

    // Initialize server modules. Tolerate per-module init failures
    // (e.g. llm_local_runtime's binary-cache setup on non-APFS) — log
    // + continue, because the routes are already registered via the
    // distributed-slice ModuleEntry list. Mirrors the server-side
    // openapi-gen path.
    // ServerConfig into the framework context; full Config via the opaque slot.
    let module_context = ziee::ServerContext::new(
        pool.clone(),
        std::sync::Arc::new(config.server_config.clone()),
        std::sync::Arc::new(config.clone()),
    );
    let mut server_modules = ziee::create_modules();
    for module in server_modules.iter_mut() {
        if let Err(e) = module.init(&module_context) {
            eprintln!(
                "openapi-gen: module '{}' init failed: {} (continuing)",
                module.name(),
                e
            );
        }
    }

    // Build server API router (returns combined router + OpenAPI doc)
    let (server_router, api_doc) =
        ziee::build_api_router(&server_modules, &config.server.api_prefix, (*pool).clone());

    // Build desktop API routes (these already include /api prefix in their paths)
    let desktop_modules = core::create_desktop_modules(None);
    let desktop_router = core::build_desktop_api_routes(&desktop_modules);

    // Merge desktop routes into server router
    let combined_router = server_router.merge(desktop_router);

    // Hand off to the framework's parameterized emit TAIL (Chunk B6): it
    // finishes the API, writes `openapi.json` into `output_dir`, and emits
    // `types.ts` via the shared (framework-hosted) generator. `output_dir` is
    // `desktop/ui/openapi`, so `types.ts` lands at
    // `desktop/ui/src/api-client/types.ts` — the formerly-hardcoded relative
    // path, now supplied by the app at the call site.
    let output_path = Path::new(output_dir);
    let types_ts_path = output_path.join("../src/api-client/types.ts");
    ziee::finish_and_emit(combined_router, api_doc, output_path, &types_ts_path)?;

    Ok(())
}
