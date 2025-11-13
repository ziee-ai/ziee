use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub postgresql: PostgreSqlConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
    pub jwt: JwtConfig,
    #[serde(default)]
    pub app: Option<AppConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub data_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgreSqlConfig {
    pub use_embedded: bool,
    #[serde(default)]
    pub embedded: Option<EmbeddedPostgreSqlConfig>,
    #[serde(default)]
    pub external: Option<ExternalPostgreSqlConfig>,
    #[serde(default)]
    pub pool: Option<PoolConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EmbeddedPostgreSqlConfig {
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
    pub logging: LoggingConfigPostgres,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExternalPostgreSqlConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfigPostgres {
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
    #[serde(default)]
    pub idle_timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_lifetime_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_prefix: String,
    #[serde(default)]
    pub cors: Option<CorsConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    pub audience: String,
    pub access_token_expiry_hours: i64,
    #[serde(default = "default_refresh_token_expiry")]
    pub refresh_token_expiry_days: i64,
}

fn default_refresh_token_expiry() -> i64 {
    30
}

impl Config {
    pub fn load_from(config_path: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Get config file path from parameter or environment variable
        let config_path = config_path
            .or_else(|| std::env::var("CONFIG_FILE").ok())
            .ok_or("Config file path not provided. Use --config-file argument or set CONFIG_FILE environment variable (e.g., CONFIG_FILE=config/dev.yaml)")?;

        tracing::info!("Loading configuration from: {}", config_path);

        // Read the file
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file '{}': {}", config_path, e))?;

        // Parse YAML
        let mut config: Config = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file '{}': {}", config_path, e))?;

        // Validate configuration
        if config.postgresql.use_embedded && config.postgresql.embedded.is_none() {
            return Err("use_embedded is true but embedded configuration is missing".into());
        }
        if !config.postgresql.use_embedded && config.postgresql.external.is_none() {
            return Err("use_embedded is false but external configuration is missing".into());
        }

        // Handle automatic port assignment if port is 0
        if config.postgresql.use_embedded {
            if let Some(ref mut embedded) = config.postgresql.embedded {
                if embedded.port == 0 {
                    embedded.port = find_available_port(50000, 50099)
                        .ok_or("Failed to find available port for database")?;
                    tracing::info!("Auto-assigned database port: {}", embedded.port);
                }
            }
        }

        if config.server.port == 0 {
            config.server.port = find_available_port(3000, 3099)
                .ok_or("Failed to find available port for server")?;
            tracing::info!("Auto-assigned server port: {}", config.server.port);
        }

        Ok(config)
    }

    pub fn database_url(&self) -> String {
        if self.postgresql.use_embedded {
            let embedded = self.postgresql.embedded.as_ref()
                .expect("embedded config must be present when use_embedded is true");
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                embedded.username,
                embedded.password,
                embedded.bind_address,
                embedded.port,
                embedded.database
            )
        } else {
            let external = self.postgresql.external.as_ref()
                .expect("external config must be present when use_embedded is false");
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                external.username,
                external.password,
                external.host,
                external.port,
                external.database
            )
        }
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

/// Find an available port in the given range
fn find_available_port(start_port: u16, end_port: u16) -> Option<u16> {
    use std::net::{SocketAddr, TcpListener};

    for port in start_port..=end_port {
        if let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
            drop(listener);
            // Double-check with a second attempt
            if let Ok(listener2) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
                drop(listener2);
                return Some(port);
            }
        }
    }

    // Fallback to portpicker if range is exhausted
    portpicker::pick_unused_port()
}
