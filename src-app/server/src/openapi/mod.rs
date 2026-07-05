use crate::core::app_builder;
use crate::core::config::Config;
use crate::module_api::ModuleContext;
use sqlx::postgres::PgPoolOptions;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub mod emit_ts;

/// Generate OpenAPI specification in the output directory
pub async fn generate_openapi_spec(
    output_dir: &str,
    config_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Generating OpenAPI specification...");

    // Load configuration
    let config = Config::load_from(config_file)?;

    // Publish the resolved data-dir + caches config into global state before
    // module init. Module `init` hooks (e.g. DeploymentManager::new) read these
    // globals via `get_caches_config()`; without this they see the empty
    // default and panic. Mirrors the boot path in `main.rs` / `lib.rs`.
    if let Some(app) = &config.app {
        crate::core::set_app_data_dir(std::path::PathBuf::from(&app.data_dir));
    }
    crate::core::set_caches_config(config.caches.clone());

    // SECURITY/PERFORMANCE: OpenAPI generation walks the router structure
    // but never executes handlers. The previous implementation called
    // initialize_database which boots the full embedded PostgreSQL (10+
    // seconds, spawn process, wait for ready, run migrations) just to
    // print a static doc. The fix uses a lazy pool that never actually
    // connects — the URL is parsed at construction but no socket opens
    // until first query, and we never issue one. Closes 14-core F-14
    // (Medium).
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&config.database_url())?,
    );

    // Initialize global repository factory
    crate::core::init_repositories((*pool).clone());

    // Module init reads `get_caches_config()` for default paths
    // (e.g. memory's embedding-engine probe touches llm_engines_dir).
    // Without this set, the unwrap inside `CachesConfig::*` panics.
    crate::core::set_caches_config(config.caches.clone());

    // Initialize modules using shared builder functions
    let module_context = ModuleContext::new(pool.clone(), std::sync::Arc::new(config.clone()));
    let mut modules = app_builder::create_modules();

    // Initialize all modules. OpenAPI generation only walks the
    // router structure — it never executes handlers — so a module
    // that fails to initialize on the current platform (e.g.
    // llm_local_runtime's binary-cache setup on a non-APFS volume)
    // shouldn't block doc generation. Log + continue.
    for module in modules.iter_mut() {
        if let Err(e) = module.init(&module_context) {
            eprintln!(
                "openapi-gen: module '{}' init failed: {} (continuing — routes are still registered)",
                module.name(),
                e
            );
        }
    }

    // Build API router using shared builder function
    // build_api_router expects PgPool, so we need to extract it from Arc
    let (api_router, mut api_doc) =
        app_builder::build_api_router(&modules, &config.server.api_prefix, (*pool).clone());

    // Finish the API and extract the OpenAPI spec
    let _router = api_router.finish_api(&mut api_doc);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&api_doc)?;

    // Ensure output directory exists
    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)?;
    }

    // Write openapi.json
    let openapi_json_path = output_path.join("openapi.json");
    fs::write(&openapi_json_path, &json)?;
    println!(
        "✓ OpenAPI specification written to: {}",
        openapi_json_path.display()
    );

    // Generate TypeScript types directly (Rust port of the former
    // `ui/openapi/generate-endpoints.ts`). `output_dir` is `ui/openapi`, so
    // `types.ts` lands at `ui/src/api-client/types.ts`.
    let types_ts = emit_ts::generate_types_ts_from_json(&json)?;
    let types_ts_path = output_path.join("../src/api-client/types.ts");
    if let Some(parent) = types_ts_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&types_ts_path, &types_ts)?;
    println!(
        "✓ TypeScript types written to: {}",
        types_ts_path.display()
    );

    println!("\n✓ OpenAPI generation complete!");
    println!("  - OpenAPI spec: {}", openapi_json_path.display());
    println!("  - TypeScript types: {}", types_ts_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end drive of `generate_openapi_spec` with a lazy (never-connected)
    /// external pool — the same path `just openapi-regen` uses. Asserts the full
    /// pipeline runs without a live DB (lazy pool + module-init-continue-on-error)
    /// and emits BOTH a well-formed `openapi.json` (with populated `paths`) and
    /// the sibling `types.ts`.
    #[tokio::test]
    async fn generates_spec_and_types_without_live_db() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let out_dir = tmp.path().join("openapi");
        std::fs::create_dir_all(&out_dir).unwrap();

        // Minimal config (mirrors config/openapi-gen.yaml). The external DB URL
        // is parsed but never connected — `connect_lazy` opens no socket and
        // generation only walks the router, so :54321 need not be up.
        let config_yaml = format!(
            r#"app:
  data_dir: "{data_dir}"

postgresql:
  use_embedded: false
  external:
    host: "127.0.0.1"
    port: 54321
    username: "postgres"
    password: "password"
    database: "postgres"
  pool:
    max_connections: 2
    min_connections: 1
    acquire_timeout_secs: 5
    idle_timeout_secs: 30
    max_lifetime_secs: 300

server:
  host: "127.0.0.1"
  port: 0
  api_prefix: "/api"

logging:
  level: "error"
  format: "pretty"

jwt:
  secret: "test-secret-change-in-production-min-32-chars-long"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30

update_check:
  enabled: false
"#,
            data_dir = data_dir.display(),
        );
        let config_path = tmp.path().join("openapi-test.yaml");
        std::fs::write(&config_path, config_yaml).unwrap();

        generate_openapi_spec(
            out_dir.to_str().unwrap(),
            Some(config_path.to_str().unwrap().to_string()),
        )
        .await
        .expect("openapi generation should succeed with a lazy pool");

        // openapi.json exists and is a valid spec with a non-empty path set.
        let spec_path = out_dir.join("openapi.json");
        assert!(spec_path.exists(), "openapi.json must be written");
        let spec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&spec_path).unwrap()).unwrap();
        assert!(
            spec.get("openapi").and_then(|v| v.as_str()).is_some(),
            "spec must carry an `openapi` version field"
        );
        let paths = spec
            .get("paths")
            .and_then(|p| p.as_object())
            .expect("spec must have a paths object");
        assert!(!paths.is_empty(), "module routes should register paths");

        // types.ts is emitted alongside (at ../src/api-client/types.ts).
        let types_path = out_dir.join("../src/api-client/types.ts");
        assert!(types_path.exists(), "types.ts must be written");
        assert!(
            !std::fs::read_to_string(&types_path).unwrap().is_empty(),
            "types.ts must be non-empty"
        );
    }
}
