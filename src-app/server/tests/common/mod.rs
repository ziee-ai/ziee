use rand::Rng;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;
use uuid::Uuid;

// Test helpers for OAuth and LDAP mock servers
pub mod ldap_mock;
pub mod oauth_mock;

/// Get database URL from environment or use default
fn database_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string())
}

pub struct TestServer {
    process: Child,
    pub base_url: String,
    pub database_name: String,
    pub database_url: String,
    temp_config_path: String,
}

/// Options for spinning up a TestServer with non-default features.
///
/// Default = `code_sandbox.enabled: false`, matching legacy behavior.
/// Tier-6 HTTP-E2E tests opt into sandbox enablement via
/// `start_with_options`; the test MUST call
/// `harness::skip_if_no_rootfs()` first to skip cleanly when bwrap or
/// the rootfs aren't available on this host.
#[derive(Debug, Clone, Default)]
pub struct TestServerOptions {
    /// Enable code_sandbox in the test server's config. Requires bwrap
    /// installed AND `sandbox_rootfs` pointing at a mounted rootfs.
    pub sandbox_enabled: bool,
    /// Rootfs path written into the test config when sandbox_enabled.
    /// Tests should pass `harness::rootfs_path()`.
    pub sandbox_rootfs: Option<PathBuf>,
    /// Cgroup parent path. Empty = rlimits-only mode (no cgroup
    /// delegation needed, runs fine in plain containers / dev shells).
    /// Tests that need cgroup enforcement should set this AND call
    /// `harness::needs_cgroup_delegation()` first.
    pub sandbox_cgroup_parent: String,
    /// Extra env vars to set on the spawned server process (e.g.
    /// `ANTHROPIC_API_KEY` for Tier-5 LLM tests). Honored verbatim;
    /// the test is responsible for unsetting sensitive ones it didn't
    /// intend to expose.
    pub extra_env: Vec<(String, String)>,
}

impl TestServer {
    /// Start a TestServer with the default options (sandbox disabled).
    /// Equivalent to `start_with_options(TestServerOptions::default())`.
    pub async fn start() -> Self {
        Self::start_with_options(TestServerOptions::default()).await
    }

    /// Start a TestServer with the given options. Use this when a test
    /// needs the code_sandbox enabled or wants to inject extra env.
    pub async fn start_with_options(opts: TestServerOptions) -> Self {
        // Generate unique identifiers
        let test_id = Uuid::new_v4().to_string();
        let database_name = format!("test_db_{}", test_id.replace("-", "_"));
        let server_port = rand::rng().random_range(10000..60000);

        // Parse DATABASE_URL to extract connection details
        let db_url = database_url();
        let url = url::Url::parse(&db_url).expect("Invalid DATABASE_URL");

        let host = url.host_str().unwrap_or("127.0.0.1");
        let port = url.port().unwrap_or(54321);
        let username = url.username();
        let password = url.password().unwrap_or("");

        // Create test config for the server
        let mut config = format!(
            r#"
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

jwt:
  # Must match the production issuer/audience because the MCP client
  # (modules/mcp/client/manager.rs) hardcodes these values when minting
  # JWTs for built-in MCP servers (code_sandbox loopback). If the
  # TestServer used different values, the validator (JwtService) would
  # reject the MCP client's tokens with InvalidIssuer and Tier-5 tests
  # (LLM → sandbox via MCP) would fail with "no tools available".
  secret: "test-secret-key-for-jwt-tokens-min-32-chars-long"
  issuer: "ziee-chat"
  audience: "ziee-chat-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
"#,
            host, port, username, password, database_name, server_port
        );

        // Optional code_sandbox section. Only written when the test
        // explicitly opts in; otherwise the server boots with sandbox
        // disabled (the default behavior every existing test relies on).
        if opts.sandbox_enabled {
            let rootfs = opts
                .sandbox_rootfs
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| {
                    panic!(
                        "TestServerOptions.sandbox_enabled=true requires \
                         sandbox_rootfs to be set. Call \
                         harness::rootfs_path() and skip the test if it \
                         returns None."
                    )
                });
            config.push_str(&format!(
                "\ncode_sandbox:\n  enabled: true\n  rootfs_path: \"{}\"\n  cgroup_parent: \"{}\"\n",
                rootfs, opts.sandbox_cgroup_parent
            ));
        }

        // Write temporary config file
        let temp_config_path = format!("/tmp/ziee-chat-test-{}.yaml", test_id);
        fs::write(&temp_config_path, config).expect("Failed to write temporary config");

        // Create the test database
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to PostgreSQL - ensure docker compose is running");

        sqlx::query(&format!("CREATE DATABASE {}", database_name))
            .execute(&pool)
            .await
            .expect("Failed to create test database");

        pool.close().await;

        // Start the server process with the temporary config
        let binary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/ziee-chat");

        let mut cmd = Command::new(binary_path);
        cmd.arg("--config-file").arg(&temp_config_path);
        for (k, v) in &opts.extra_env {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("Failed to start test server");

        // Construct base URL
        let base_url = format!("http://127.0.0.1:{}", server_port);

        // Construct database URL for the test database
        let test_database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            username, password, host, port, database_name
        );

        // Wait for server to be ready
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
            database_url: test_database_url,
            temp_config_path,
        }
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

        // Cleanup database
        let database_name = self.database_name.clone();
        let db_url = database_url();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _ = handle.spawn(async move {
                if let Ok(pool) = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&db_url)
                    .await
                {
                    // Terminate existing connections
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
    }
}

/// Common test helpers for creating users and managing permissions
pub mod test_helpers {
    use super::TestServer;
    use serde_json::json;
    use uuid::Uuid;

    /// Test user with token and ID
    #[derive(Debug, Clone)]
    pub struct TestUser {
        pub token: String,
        pub user_id: String,
    }

    /// Create a user with specific permissions for testing
    pub async fn create_user_with_permissions(
        server: &TestServer,
        username: &str,
        permissions: &[&str],
    ) -> TestUser {
        let unique_username = format!("{}_{}", username, &Uuid::new_v4().to_string()[..8]);

        // Register user via API
        let register_response = reqwest::Client::new()
            .post(&server.api_url("/auth/register"))
            .json(&json!({
                "username": &unique_username,
                "email": format!("{}@example.com", unique_username),
                "password": "password123"
            }))
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(
            register_response.status(),
            201,
            "Registration should succeed"
        );

        let register_body: serde_json::Value = register_response
            .json()
            .await
            .expect("Failed to parse register response");

        let token = register_body["access_token"]
            .as_str()
            .expect("access_token missing")
            .to_string();
        let user_id = register_body["user"]["id"]
            .as_str()
            .expect("user id missing")
            .to_string();

        // Assign permissions if needed
        if !permissions.is_empty() {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&server.database_url)
                .await
                .expect("Failed to connect to test database");

            let group_id = Uuid::new_v4();
            let group_name = format!("test_group_{}", &group_id.to_string()[..8]);
            let permissions_json: Vec<String> = permissions.iter().map(|s| s.to_string()).collect();

            sqlx::query(
                "INSERT INTO groups (id, name, description, permissions, is_system, is_active, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, false, true, NOW(), NOW())"
            )
            .bind(group_id)
            .bind(&group_name)
            .bind("Test group for permissions")
            .bind(&permissions_json)
            .execute(&pool)
            .await
            .expect("Failed to create test group");

            // Assign user to custom permissions group
            let user_uuid = Uuid::parse_str(&user_id).expect("Invalid user ID");
            sqlx::query(
                "INSERT INTO user_groups (user_id, group_id, assigned_at)
                 VALUES ($1, $2, NOW())",
            )
            .bind(user_uuid)
            .bind(group_id)
            .execute(&pool)
            .await
            .expect("Failed to assign user to custom group");

            // Also assign user to default group (like real registration does)
            let default_group_result =
                sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
                    .fetch_optional(&pool)
                    .await
                    .expect("Failed to query default group");

            if let Some(default_group) = default_group_result {
                sqlx::query(
                    "INSERT INTO user_groups (user_id, group_id, assigned_at)
                     VALUES ($1, $2, NOW())
                     ON CONFLICT DO NOTHING",
                )
                .bind(user_uuid)
                .bind(default_group.id)
                .execute(&pool)
                .await
                .expect("Failed to assign user to default group");
            }

            pool.close().await;
        }

        TestUser { token, user_id }
    }

    /// Create a user that has NO permissions at all — including no membership
    /// in the default "Users" group (which grants `mcp_servers::*`, `chat::*`,
    /// etc. via migration 27).
    ///
    /// Use this for route-level "should return 403" tests where you need to
    /// prove the authorization gate works. `create_user_with_permissions(_, _, &[])`
    /// is NOT suitable — registration auto-assigns the default group, so the
    /// resulting user actually has a broad set of inherited permissions.
    pub async fn create_user_with_no_permissions(
        server: &TestServer,
        username: &str,
    ) -> TestUser {
        let user = create_user_with_permissions(server, username, &[]).await;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&server.database_url)
            .await
            .expect("Failed to connect to test database");

        let user_uuid = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
        sqlx::query("DELETE FROM user_groups WHERE user_id = $1")
            .bind(user_uuid)
            .execute(&pool)
            .await
            .expect("Failed to strip user from groups");

        pool.close().await;
        user
    }

    /// Create a test user via API (requires admin token)
    pub async fn create_test_user(
        server: &TestServer,
        admin_token: &str,
        username: &str,
        password: &str,
    ) -> serde_json::Value {
        let url = server.api_url("/users");
        let payload = json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": password
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin_token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Failed to create test user");
        response.json().await.expect("Failed to parse JSON")
    }
}

/// Helper to make HTTP requests during tests
pub mod http {
    use serde::Serialize;
    use serde::de::DeserializeOwned;

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

    pub async fn delete_with_auth(
        url: &str,
        token: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let client = reqwest::Client::new();
        client
            .delete(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
    }
}
