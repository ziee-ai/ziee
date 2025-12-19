//! OpenAPI Generation
//!
//! Generates combined OpenAPI specification including both server and desktop endpoints.

use crate::core;
use std::fs;
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
    let config = ziee_chat::Config::load_from(config_file)?;

    // Initialize database (this starts embedded PostgreSQL if use_embedded: true)
    let pool = ziee_chat::initialize_database(&config).await?;

    // Initialize global repository factory
    ziee_chat::init_repositories((*pool).clone());

    // Initialize server modules
    let module_context = ziee_chat::ServerContext::new(pool.clone(), std::sync::Arc::new(config.clone()));
    let mut server_modules = ziee_chat::create_modules();
    ziee_chat::initialize_modules(&mut server_modules, &module_context)?;

    // Build server API router (returns combined router + OpenAPI doc)
    let (server_router, mut api_doc) =
        ziee_chat::build_api_router(&server_modules, &config.server.api_prefix, (*pool).clone());

    // Build desktop API routes (these already include /api prefix in their paths)
    let desktop_modules = core::create_desktop_modules(None);
    let desktop_router = core::build_desktop_api_routes(&desktop_modules);

    // Merge desktop routes into server router
    let combined_router = server_router.merge(desktop_router);

    // Finish the API and extract the OpenAPI spec
    let _router = combined_router.finish_api(&mut api_doc);

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

    println!("\n✓ OpenAPI generation complete!");
    println!("  - OpenAPI spec: {}", openapi_json_path.display());
    println!("\nTo generate TypeScript types, run:");
    println!("  cd ui && npm run generate-openapi");

    Ok(())
}
