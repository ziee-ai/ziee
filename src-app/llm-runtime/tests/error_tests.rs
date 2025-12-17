//! Error handling tests

use llm_runtime::{RuntimeError, Result};

#[test]
fn test_error_types() {
    // Test all error variants can be created
    let _ = RuntimeError::config("test");
    let _ = RuntimeError::startup_failed("test");
    let _ = RuntimeError::health_check_failed("test");
    let _ = RuntimeError::shutdown_failed("test");
    let _ = RuntimeError::network("test");
    let _ = RuntimeError::timeout("test");
    let _ = RuntimeError::internal("test");
}

#[test]
fn test_error_display() {
    let err = RuntimeError::config("invalid setting");
    assert_eq!(err.to_string(), "Configuration error: invalid setting");

    let err = RuntimeError::StartupFailed("engine crashed".to_string());
    assert_eq!(err.to_string(), "Engine startup failed: engine crashed");

    let err = RuntimeError::HealthCheckFailed("timeout".to_string());
    assert_eq!(err.to_string(), "Health check failed: timeout");
}

#[test]
fn test_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let runtime_err: RuntimeError = io_err.into();

    assert!(matches!(runtime_err, RuntimeError::Io(_)));
}

#[test]
fn test_error_from_yaml() {
    let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("invalid: [").unwrap_err();
    let runtime_err: RuntimeError = yaml_err.into();

    assert!(matches!(runtime_err, RuntimeError::YamlParse(_)));
}

#[test]
fn test_error_result_type() {
    fn returns_error() -> Result<()> {
        Err(RuntimeError::config("test error"))
    }

    let result = returns_error();
    assert!(result.is_err());

    match result {
        Err(RuntimeError::Config(msg)) => assert_eq!(msg, "test error"),
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_instance_not_found_error() {
    let err = RuntimeError::InstanceNotFound("my-model".to_string());
    assert_eq!(err.to_string(), "Instance not found: my-model");
}

#[test]
fn test_instance_already_exists_error() {
    let err = RuntimeError::InstanceAlreadyExists("my-model".to_string());
    assert_eq!(err.to_string(), "Instance already exists: my-model");
}

#[test]
fn test_binary_not_found_error() {
    let err = RuntimeError::BinaryNotFound("llama-server".to_string());
    assert_eq!(err.to_string(), "Binary not found or not executable: llama-server");
}

#[test]
fn test_port_unavailable_error() {
    let err = RuntimeError::PortUnavailable("all ports in use".to_string());
    assert_eq!(err.to_string(), "Port unavailable: all ports in use");
}
