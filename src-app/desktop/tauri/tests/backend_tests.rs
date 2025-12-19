//! Backend module integration tests
//!
//! Tests for the embedded server management functionality

mod common;

use serial_test::serial;
use std::time::Duration;

/// Test that the backend module can find an available port
#[test]
fn test_find_available_port() {
    let port = common::find_available_port(19000, 19100);
    assert!(port.is_some(), "Should find an available port");

    let port = port.unwrap();
    assert!(port >= 19000 && port < 19100, "Port should be in range");
}

/// Test that the test config generates valid configuration
#[test]
fn test_config_generation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = common::TestConfig::new(temp_dir.path().to_path_buf());

    assert!(config.server_port >= 18080 && config.server_port < 18180);
    assert_eq!(config.data_dir, temp_dir.path());
}

/// Test that server config YAML is properly formatted
#[test]
fn test_server_config_yaml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = common::TestConfig::new(temp_dir.path().to_path_buf());
    let yaml = common::server::generate_test_config(&config);

    // Verify YAML contains expected sections
    assert!(yaml.contains("server:"));
    assert!(yaml.contains("database:"));
    assert!(yaml.contains("auth:"));
    assert!(yaml.contains(&format!("port: {}", config.server_port)));
}

// Note: Full server startup tests require the embedded PostgreSQL setup
// and are marked with #[serial] to avoid port conflicts

/// Test server readiness check with non-existent server
#[tokio::test]
#[serial]
async fn test_wait_for_server_timeout() {
    // Use a port that's definitely not in use
    let result = common::server::wait_for_server_ready(
        19999,
        Duration::from_millis(500),
    ).await;

    assert!(!result, "Should timeout when server is not running");
}

// Full integration test that starts the actual server
// This test is expensive and should be run selectively
#[tokio::test]
#[serial]
#[ignore = "Requires full server startup - run with --ignored"]
async fn test_full_server_lifecycle() {
    use common::server::wait_for_server_ready;

    let temp_dir = tempfile::tempdir().unwrap();
    let config = common::TestConfig::new(temp_dir.path().to_path_buf());

    // In a real test, we would:
    // 1. Start the server with the generated config
    // 2. Wait for it to be ready
    // 3. Test various endpoints
    // 4. Shut down gracefully

    // For now, verify the setup is correct
    assert!(config.server_port > 0);

    // This would be the actual server startup test
    // let server_ready = wait_for_server_ready(
    //     config.server_port,
    //     Duration::from_secs(60),
    // ).await;
    // assert!(server_ready, "Server should start within timeout");
}
