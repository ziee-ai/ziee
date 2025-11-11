//! Error types for AI provider operations

use thiserror::Error;

/// Errors that can occur when interacting with AI providers
#[derive(Error, Debug)]
pub enum ProviderError {
    /// Network or HTTP request error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// OpenAI library error
    #[error("OpenAI error: {0}")]
    OpenAI(String),

    /// Gemini library error
    #[error("Gemini error: {0}")]
    Gemini(String),

    /// Authentication failed (invalid API key)
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Provider-specific error
    #[error("Provider error: {0}")]
    ProviderSpecific(String),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// File upload failed
    #[error("File upload failed: {0}")]
    FileUpload(String),

    /// Streaming error
    #[error("Streaming error: {0}")]
    Streaming(String),

    /// Feature not supported by this provider
    #[error("Feature not supported: {0}")]
    NotSupported(String),

    /// Timeout error
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl ProviderError {
    /// Creates an authentication error
    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    /// Creates a rate limit error
    pub fn rate_limit(msg: impl Into<String>) -> Self {
        Self::RateLimit(msg.into())
    }

    /// Creates an invalid request error
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::InvalidRequest(msg.into())
    }

    /// Creates a provider-specific error
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::ProviderSpecific(msg.into())
    }

    /// Creates a file upload error
    pub fn file_upload(msg: impl Into<String>) -> Self {
        Self::FileUpload(msg.into())
    }

    /// Creates a streaming error
    pub fn streaming(msg: impl Into<String>) -> Self {
        Self::Streaming(msg.into())
    }

    /// Creates a not supported error
    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::NotSupported(msg.into())
    }

    /// Creates a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// Creates an unknown error
    pub fn unknown(msg: impl Into<String>) -> Self {
        Self::Unknown(msg.into())
    }

    /// Parses HTTP status codes into appropriate error types
    pub fn from_status_code(status: u16, body: String) -> Self {
        match status {
            401 => Self::auth(format!("Unauthorized: {}", body)),
            429 => Self::rate_limit(format!("Too many requests: {}", body)),
            400 => Self::invalid_request(format!("Bad request: {}", body)),
            408 | 504 => Self::timeout(format!("Request timeout: {}", body)),
            _ => Self::provider(format!("HTTP {}: {}", status, body)),
        }
    }
}

// Note: OpenAI uses custom implementation, errors are handled directly

/// Convert from Gemini library errors
impl From<gemini_rust::ClientError> for ProviderError {
    fn from(e: gemini_rust::ClientError) -> Self {
        use gemini_rust::ClientError;
        match e {
            ClientError::InvalidApiKey { .. } => {
                Self::Authentication("Invalid Gemini API key".to_string())
            }
            ClientError::BadResponse { code, description } => {
                Self::from_status_code(code, description.unwrap_or_default())
            }
            ClientError::PerformRequest { source, .. } => {
                Self::Network(source)
            }
            _ => Self::Gemini(e.to_string()),
        }
    }
}
