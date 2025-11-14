use crate::core::app_builder;
use crate::core::config::Config;
use crate::module_api::ModuleContext;
use std::fs;
use std::path::Path;

/// Generate OpenAPI specification in the output directory
pub async fn generate_openapi_spec(
    output_dir: &str,
    config_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Generating OpenAPI specification...");

    // Load configuration
    let config = Config::load_from(config_file)?;

    // Initialize database properly (this starts embedded PostgreSQL if use_embedded: true)
    let pool = crate::core::database::initialize_database(&config).await?;

    // Initialize global repository factory
    crate::core::init_repositories((*pool).clone());

    // Initialize modules using shared builder functions
    let module_context = ModuleContext::new(pool.clone(), std::sync::Arc::new(config.clone()));
    let mut modules = app_builder::create_modules();

    // Initialize all modules
    app_builder::initialize_modules(&mut modules, &module_context)?;

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

    println!("\n✓ OpenAPI generation complete!");
    println!("  - OpenAPI spec: {}", openapi_json_path.display());
    println!("\nTo generate TypeScript types, run:");
    println!("  cd ui && npm run generate-openapi");

    Ok(())
}
