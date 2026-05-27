// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ziee-desktop")]
#[command(version, about = "Ziee Chat Desktop Application", long_about = None)]
struct Cli {
    /// Path to configuration file (overrides CONFIG_FILE env var)
    #[arg(long, value_name = "FILE")]
    config_file: Option<String>,

    /// Generate OpenAPI specification (server + desktop endpoints), then exit
    /// If no value is provided, defaults to ../ui/openapi (desktop UI)
    #[arg(long, value_name = "OUTPUT_DIR", num_args = 0..=1, default_missing_value = "../ui/openapi")]
    generate_openapi: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Check for OpenAPI generation flag
    if cli.generate_openapi.is_some() {
        let output_dir = cli.generate_openapi.unwrap_or_else(|| {
            // Default to desktop/ui/openapi relative to the tauri directory
            match option_env!("CARGO_MANIFEST_DIR") {
                Some(manifest_dir) => format!("{}/../ui/openapi", manifest_dir),
                None => {
                    eprintln!("Please specify an output directory explicitly:");
                    eprintln!("  --generate-openapi /path/to/output");
                    std::process::exit(1);
                }
            }
        });

        // Run OpenAPI generation
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        match rt.block_on(ziee_desktop::openapi::generate_openapi_spec(&output_dir, cli.config_file)) {
            Ok(_) => {
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error generating OpenAPI spec: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Get config file from CLI arg or CONFIG_FILE env var
    let config_file = cli.config_file.or_else(|| std::env::var("CONFIG_FILE").ok());

    ziee_desktop::run(config_file).expect("Failed to run desktop app");
}
