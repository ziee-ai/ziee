use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub postgresql: PostgreSqlConfig,
    pub migrations: MigrationsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgreSqlConfig {
    pub version: String,
    pub port: u16,
    pub bind_address: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub installation_dir: String,
    pub data_dir: String,
    pub timezone: String,
    pub log_timezone: String,
    pub logging: LoggingConfig,
    pub pool: PoolConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub collector: bool,
    pub directory: String,
    pub filename: String,
    pub statement: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MigrationsConfig {
    pub path: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Get config file path from environment variable or use default
        let config_path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| {
            let mut default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            default_path.push("../config/build.yaml");
            default_path.to_string_lossy().to_string()
        });

        println!("Loading configuration from: {}", config_path);

        // Read the file
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file '{}': {}", config_path, e))?;

        // Parse YAML
        let config: Config = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file '{}': {}", config_path, e))?;

        Ok(config)
    }

    pub fn installation_dir(&self) -> PathBuf {
        PathBuf::from(&self.postgresql.installation_dir)
    }

    pub fn data_dir(&self) -> PathBuf {
        PathBuf::from(&self.postgresql.data_dir)
    }

    pub fn migrations_path(&self) -> PathBuf {
        PathBuf::from(&self.migrations.path)
    }
}
