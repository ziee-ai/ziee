use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Get config file path from BUILD_CONFIG_FILE or use default
    let config_path = env::var("BUILD_CONFIG_FILE").unwrap_or_else(|_| {
        let mut default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        default_path.push("../config/build.yaml");
        default_path.to_string_lossy().to_string()
    });

    println!("cargo:rerun-if-env-changed=BUILD_CONFIG_FILE");
    println!("cargo:rerun-if-changed={}", config_path);

    // Read and parse the config file
    let config_content = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config file '{}': {}", config_path, e));

    let config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .unwrap_or_else(|e| panic!("Failed to parse config file '{}': {}", config_path, e));

    // Extract PostgreSQL configuration
    // build.yaml always uses embedded PostgreSQL with a simple flat structure
    let postgresql = config
        .get("postgresql")
        .expect("postgresql section not found in config");

    let username = postgresql
        .get("username")
        .and_then(|v| v.as_str())
        .expect("postgresql.username not found");
    let password = postgresql
        .get("password")
        .and_then(|v| v.as_str())
        .expect("postgresql.password not found");
    let bind_address = postgresql
        .get("bind_address")
        .and_then(|v| v.as_str())
        .unwrap_or("127.0.0.1");
    let port = postgresql
        .get("port")
        .and_then(|v| v.as_u64())
        .expect("postgresql.port not found");
    let database = postgresql
        .get("database")
        .and_then(|v| v.as_str())
        .expect("postgresql.database not found");

    let database_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        username, password, bind_address, port, database
    );

    // Set DATABASE_URL as a compile-time environment variable for SQLx macros
    println!("cargo:rustc-env=DATABASE_URL={}", database_url);
}
