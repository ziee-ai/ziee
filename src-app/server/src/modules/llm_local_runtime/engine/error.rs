//! Error types for the LLM runtime

use std::fmt;

/// Result type alias for runtime operations
pub type Result<T> = std::result::Result<T, RuntimeError>;

/// Errors that can occur during runtime operations
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Configuration error (invalid YAML, missing required fields, etc.)
    #[error("Configuration error: {0}")]
    Config(String),

    /// Engine type not found or not supported
    #[error("Engine not found: {0}")]
    EngineNotFound(String),

    /// Instance ID not found in registry
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    /// Instance with this ID already exists
    #[error("Instance already exists: {0}")]
    InstanceAlreadyExists(String),

    /// Engine binary not found or not executable
    #[error("Binary not found or not executable: {0}")]
    BinaryNotFound(String),

    /// Failed to extract or cache binary
    #[error("Binary extraction failed: {0}")]
    BinaryExtractionFailed(String),

    /// Failed to start engine process
    #[error("Engine startup failed: {0}")]
    StartupFailed(String),

    /// Health check failed or timed out
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    /// Failed to stop engine process
    #[error("Engine shutdown failed: {0}")]
    ShutdownFailed(String),

    /// Port already in use or unavailable
    #[error("Port unavailable: {0}")]
    PortUnavailable(String),

    /// Network error (HTTP requests, etc.)
    #[error("Network error: {0}")]
    Network(String),

    /// IO error (file operations, process spawning, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error (for engine responses + safetensors config.json)
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Archive extraction error (ZIP)
    #[error("Archive extraction error: {0}")]
    ZipExtraction(#[from] zip::result::ZipError),

    /// Timeout error
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Internal error (should not happen)
    #[error("Internal error: {0}")]
    Internal(String),
}

impl RuntimeError {
    /// Create a config error
    pub fn config(msg: impl fmt::Display) -> Self {
        Self::Config(msg.to_string())
    }

    /// Create a startup failed error
    pub fn startup_failed(msg: impl fmt::Display) -> Self {
        Self::StartupFailed(msg.to_string())
    }

    /// Create a health check failed error
    pub fn health_check_failed(msg: impl fmt::Display) -> Self {
        Self::HealthCheckFailed(msg.to_string())
    }

    /// Create a shutdown failed error
    pub fn shutdown_failed(msg: impl fmt::Display) -> Self {
        Self::ShutdownFailed(msg.to_string())
    }

    /// Create a network error
    pub fn network(msg: impl fmt::Display) -> Self {
        Self::Network(msg.to_string())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl fmt::Display) -> Self {
        Self::Timeout(msg.to_string())
    }

    /// Create an internal error
    pub fn internal(msg: impl fmt::Display) -> Self {
        Self::Internal(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    // Ported from llm-runtime/tests/error_tests.rs (the `YamlParse` case
    // was dropped along with the unused serde_yaml variant).
    use super::*;

    #[test]
    fn test_error_types() {
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
    fn test_error_result_type() {
        fn returns_error() -> Result<()> {
            Err(RuntimeError::config("test error"))
        }
        match returns_error() {
            Err(RuntimeError::Config(msg)) => assert_eq!(msg, "test error"),
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_named_variant_displays() {
        assert_eq!(
            RuntimeError::InstanceNotFound("my-model".to_string()).to_string(),
            "Instance not found: my-model"
        );
        assert_eq!(
            RuntimeError::BinaryNotFound("llama-server".to_string()).to_string(),
            "Binary not found or not executable: llama-server"
        );
        assert_eq!(
            RuntimeError::PortUnavailable("all ports in use".to_string()).to_string(),
            "Port unavailable: all ports in use"
        );
    }
}
