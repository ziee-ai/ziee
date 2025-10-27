use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;
use rand::Rng;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

// Static to track the shared embedded PostgreSQL instance
static SHARED_POSTGRES: std::sync::LazyLock<Mutex<Option<postgresql_embedded::PostgreSQL>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

// Test configuration loaded from test.yaml
#[derive(Debug, Clone)]
struct TestConfig {
    pg_version: String,
    pg_port: u16,
    pg_bind_address: String,
    pg_username: String,
    pg_password: String,
    pg_database: String,
    pg_installation_dir: String,
    pg_data_dir: String,
    pg_timezone: String,
    pg_log_timezone: String,
    pg_log_collector: bool,
    pg_log_directory: String,
    pg_log_filename: String,
    pg_log_statement: String,
}

impl TestConfig {
    fn load() -> Self {
        let mut config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("../config/test.yaml");
        let config_content = fs::read_to_string(&config_path)
            .expect("Failed to read test.yaml");
        let config: serde_yaml::Value = serde_yaml::from_str(&config_content)
            .expect("Failed to parse test.yaml");

        let pg = &config["postgresql"];

        Self {
            pg_version: pg["version"].as_str().unwrap().to_string(),
            pg_port: pg["port"].as_u64().unwrap() as u16,
            pg_bind_address: pg["bind_address"].as_str().unwrap().to_string(),
            pg_username: pg["username"].as_str().unwrap().to_string(),
            pg_password: pg["password"].as_str().unwrap().to_string(),
            pg_database: pg["database"].as_str().unwrap().to_string(),
            pg_installation_dir: pg["installation_dir"].as_str().unwrap().to_string(),
            pg_data_dir: pg["data_dir"].as_str().unwrap().to_string(),
            pg_timezone: pg["timezone"].as_str().unwrap().to_string(),
            pg_log_timezone: pg["log_timezone"].as_str().unwrap().to_string(),
            pg_log_collector: pg["logging"]["collector"].as_bool().unwrap(),
            pg_log_directory: pg["logging"]["directory"].as_str().unwrap().to_string(),
            pg_log_filename: pg["logging"]["filename"].as_str().unwrap().to_string(),
            pg_log_statement: pg["logging"]["statement"].as_str().unwrap().to_string(),
        }
    }

    fn database_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.pg_username,
            self.pg_password,
            self.pg_bind_address,
            self.pg_port,
            self.pg_database
        )
    }
}

static TEST_CONFIG: std::sync::LazyLock<TestConfig> =
    std::sync::LazyLock::new(|| TestConfig::load());

pub struct TestServer {
    process: Child,
    pub base_url: String,
    pub database_name: String,
    temp_config_path: String,
}

impl TestServer {
    /// Start a new test server instance with isolated database and random port
    /// This will ensure a shared embedded PostgreSQL is running first
    pub async fn start() -> Self {
        // Ensure shared PostgreSQL instance is running
        Self::ensure_shared_postgres().await;

        // Generate unique identifiers
        let test_id = Uuid::new_v4().to_string();
        let database_name = format!("test_db_{}", test_id.replace("-", "_"));
        let server_port = rand::rng().random_range(10000..60000);

        // Create a complete config for the test server
        // This server will connect to external PostgreSQL (our shared embedded instance)
        let test_config = &*TEST_CONFIG;
        let config = format!(r#"
postgresql:
  use_embedded: false

  external:
    host: "{}"
    port: {}
    username: "{}"
    password: "{}"
    database: "{}"

  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60

server:
  host: "127.0.0.1"
  port: {}
  api_prefix: "/api"
"#,
            test_config.pg_bind_address,
            test_config.pg_port,
            test_config.pg_username,
            test_config.pg_password,
            database_name,
            server_port
        );

        // Write temporary config file
        let temp_config_path = format!("/tmp/ziee-chat-test-{}.yaml", test_id);
        fs::write(&temp_config_path, config)
            .expect("Failed to write temporary config");

        // Create the test database in the shared PostgreSQL instance
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&test_config.database_url())
            .await
            .expect("Failed to connect to shared PostgreSQL");

        // Create database
        sqlx::query(&format!("CREATE DATABASE {}", database_name))
            .execute(&pool)
            .await
            .expect("Failed to create test database");

        pool.close().await;

        // Start the server process with the temporary config
        let binary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/debug/ziee-chat");

        let child = Command::new(binary_path)
            .arg("--config-file")
            .arg(&temp_config_path)
            .spawn()
            .expect("Failed to start test server");

        // Construct base URL
        let base_url = format!("http://127.0.0.1:{}", server_port);

        // Wait for server to be ready by polling the health endpoint
        let client = reqwest::Client::new();
        let health_url = format!("{}/api/health", base_url);

        for _ in 0..30 {
            if let Ok(response) = client.get(&health_url).send().await {
                if response.status().is_success() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        TestServer {
            process: child,
            base_url,
            database_name,
            temp_config_path,
        }
    }

    /// Ensure the shared embedded PostgreSQL instance is running
    async fn ensure_shared_postgres() {
        use postgresql_embedded::{PostgreSQL, Settings, VersionReq};

        let test_config = &*TEST_CONFIG;

        // Try to connect to existing PostgreSQL first
        if PgPoolOptions::new()
            .max_connections(1)
            .connect(&test_config.database_url())
            .await
            .is_ok()
        {
            // PostgreSQL is already running (possibly from previous test run)
            println!("Shared embedded PostgreSQL already running on port {}", test_config.pg_port);
            return;
        }

        // Try to acquire lock - if poisoned, it means a previous test panicked
        // but we should still try to start PostgreSQL
        let mut postgres_guard = match SHARED_POSTGRES.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                println!("Lock was poisoned, recovering...");
                poisoned.into_inner()
            }
        };

        if postgres_guard.is_some() {
            // Already running in this process
            drop(postgres_guard);
            return;
        }

        println!("Starting shared embedded PostgreSQL for tests...");

        // Use test config
        let mut settings = Settings::default();
        settings.version = VersionReq::parse(&format!("={}", test_config.pg_version)).unwrap();
        settings.temporary = false;
        settings.port = test_config.pg_port;
        settings.host = test_config.pg_bind_address.clone();
        settings.username = test_config.pg_username.clone();
        settings.password = test_config.pg_password.clone();

        settings.installation_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(&test_config.pg_installation_dir);
        settings.data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(&test_config.pg_data_dir);
        settings.password_file = settings.installation_dir.join(".pgpass");

        // Set timezone
        settings.configuration.insert(
            "timezone".to_string(),
            test_config.pg_timezone.clone()
        );
        settings.configuration.insert(
            "log_timezone".to_string(),
            test_config.pg_log_timezone.clone()
        );

        // Set logging config
        settings.configuration.insert(
            "logging_collector".to_string(),
            if test_config.pg_log_collector { "on" } else { "off" }.to_string()
        );
        settings.configuration.insert(
            "log_directory".to_string(),
            test_config.pg_log_directory.clone()
        );
        settings.configuration.insert(
            "log_filename".to_string(),
            test_config.pg_log_filename.clone()
        );
        settings.configuration.insert(
            "log_statement".to_string(),
            test_config.pg_log_statement.clone()
        );

        let mut postgresql = PostgreSQL::new(settings);

        println!("Setting up embedded PostgreSQL on port {}...", test_config.pg_port);
        postgresql.setup().await.expect("Failed to setup PostgreSQL");

        println!("Starting embedded PostgreSQL...");
        if let Err(e) = postgresql.start().await {
            println!("Warning: Failed to start PostgreSQL: {:?}", e);
            println!("Assuming PostgreSQL is already running externally on port {}", test_config.pg_port);
            drop(postgres_guard);
            return;
        }

        println!("Shared embedded PostgreSQL started successfully");

        *postgres_guard = Some(postgresql);
        drop(postgres_guard);

        // Give it a moment to be fully ready
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    /// Get the base URL for API requests
    pub fn api_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Kill the server process
        let _ = self.process.kill();
        let _ = self.process.wait();

        // Delete the temporary config file
        let _ = fs::remove_file(&self.temp_config_path);

        // Note: Database cleanup is best effort - we try to drop if there's a runtime available
        let database_name = self.database_name.clone();

        // Try to use existing runtime handle if available
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let test_config = TEST_CONFIG.clone();
            let _ = handle.spawn(async move {
                if let Ok(pool) = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&test_config.database_url())
                    .await
                {
                    // Terminate existing connections to the database
                    let _ = sqlx::query(&format!(
                        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid()",
                        database_name
                    ))
                    .execute(&pool)
                    .await;

                    // Drop the database
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {}", database_name))
                        .execute(&pool)
                        .await;

                    pool.close().await;
                }
            });
        }
        // If no runtime is available, the database will be left behind but that's okay
        // for test cleanup - the shared PostgreSQL instance will be cleaned up eventually
    }
}

/// Helper to make HTTP requests during tests
pub mod http {
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    pub async fn get<T: DeserializeOwned>(url: &str) -> Result<T, reqwest::Error> {
        reqwest::get(url).await?.json().await
    }

    pub async fn get_with_auth<T: DeserializeOwned>(
        url: &str,
        token: &str,
    ) -> Result<T, reqwest::Error> {
        let client = reqwest::Client::new();
        client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .json()
            .await
    }

    pub async fn post<T: Serialize, R: DeserializeOwned>(
        url: &str,
        body: &T,
    ) -> Result<R, reqwest::Error> {
        let client = reqwest::Client::new();
        client.post(url).json(body).send().await?.json().await
    }

    pub async fn post_with_auth<T: Serialize, R: DeserializeOwned>(
        url: &str,
        token: &str,
        body: &T,
    ) -> Result<R, reqwest::Error> {
        let client = reqwest::Client::new();
        client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .json(body)
            .send()
            .await?
            .json()
            .await
    }

    pub async fn put<T: Serialize, R: DeserializeOwned>(
        url: &str,
        body: &T,
    ) -> Result<R, reqwest::Error> {
        let client = reqwest::Client::new();
        client.put(url).json(body).send().await?.json().await
    }

    pub async fn delete(url: &str) -> Result<reqwest::Response, reqwest::Error> {
        let client = reqwest::Client::new();
        client.delete(url).send().await
    }

    pub async fn delete_with_auth(url: &str, token: &str) -> Result<reqwest::Response, reqwest::Error> {
        let client = reqwest::Client::new();
        client
            .delete(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
    }
}
