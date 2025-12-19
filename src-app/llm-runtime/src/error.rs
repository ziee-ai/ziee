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

    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// JSON parsing error (for engine responses)
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

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
