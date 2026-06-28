//! Server test utilities
//!
//! Helpers for starting and managing the embedded server during tests

use super::TestConfig;
use std::time::Duration;
use tokio::time::sleep;

/// Wait for the server to be ready by checking if the port is accepting connections
pub async fn wait_for_server_ready(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    let addr = format!("127.0.0.1:{}", port);

    while start.elapsed() < timeout {
        if std::net::TcpStream::connect(&addr).is_ok() {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }

    false
}

/// Generate a test server configuration YAML
pub fn generate_test_config(config: &TestConfig) -> String {
    format!(
        r#"
server:
  host: "127.0.0.1"
  port: {}
  api_prefix: "/api"

database:
  embedded: true
  path: "{}"

auth:
  jwt_secret: "test_secret_key_for_integration_tests_only"
  token_expiration_hours: 24

logging:
  level: "warn"
"#,
        config.server_port,
        config.data_dir.join("postgres").display()
    )
}

// HEALTH_CHECK_PATH and check_server_health removed — dead code.
