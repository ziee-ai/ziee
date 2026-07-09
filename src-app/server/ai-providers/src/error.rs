//! Error types for AI provider operations

use thiserror::Error;

/// Errors that can occur when interacting with AI providers
#[derive(Error, Debug)]
pub enum ProviderError {
    /// Network or HTTP request error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

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
}

/// Bound + sanitize an untrusted provider response body before it goes into an
/// error string (which is logged and may reach the user). A hostile/compromised
/// endpoint could otherwise return a multi-megabyte or newline-laden body to
/// bloat logs or forge entries; and a reflective endpoint could echo request
/// material. Truncates to a char boundary and collapses CR/LF to spaces.
pub(crate) fn sanitize_error_body(body: &str) -> String {
    const MAX: usize = 1024;
    let cleaned: String = body
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    match cleaned.char_indices().nth(MAX) {
        Some((i, _)) => format!("{}…[truncated]", &cleaned[..i]),
        None => cleaned,
    }
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

    /// Parses HTTP status codes into appropriate error types. The (untrusted)
    /// response `body` is bounded + sanitized before being embedded.
    pub fn from_status_code(status: u16, body: String) -> Self {
        let body = sanitize_error_body(&body);
        match status {
            401 | 403 => Self::auth(format!("Unauthorized: {}", body)),
            429 => Self::rate_limit(format!("Too many requests: {}", body)),
            400 | 404 => Self::invalid_request(format!("Bad request: {}", body)),
            408 | 504 => Self::timeout(format!("Request timeout: {}", body)),
            _ => Self::provider(format!("HTTP {}: {}", status, body)),
        }
    }

    /// Creates error from Anthropic error event
    pub fn from_anthropic_error(error_type: &str, message: &str) -> Self {
        match error_type {
            "overloaded_error" => Self::rate_limit(format!("Overloaded: {}", message)),
            "rate_limit_error" => Self::rate_limit(message),
            "authentication_error" => Self::auth(message),
            "invalid_request_error" => Self::invalid_request(message),
            "permission_error" => Self::auth(message),
            _ => Self::provider(format!("Anthropic {}: {}", error_type, message)),
        }
    }
}

// Note: All providers now use custom HTTP implementations
// Errors are handled directly via reqwest and status code parsing

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_status_code_truncates_and_strips_newlines() {
        let body = format!("oops\nsecond line\r\n{}", "x".repeat(5000));
        let msg = ProviderError::from_status_code(500, body).to_string();
        // Newlines collapsed (no log-forging) and length bounded.
        assert!(!msg.contains('\n'));
        assert!(!msg.contains('\r'));
        assert!(msg.contains("[truncated]"));
        assert!(msg.len() < 1200);
    }

    #[test]
    fn from_status_code_maps_403_and_404() {
        assert!(matches!(
            ProviderError::from_status_code(403, String::new()),
            ProviderError::Authentication(_)
        ));
        assert!(matches!(
            ProviderError::from_status_code(404, String::new()),
            ProviderError::InvalidRequest(_)
        ));
    }

    #[test]
    fn sanitize_body_short_input_unchanged() {
        assert_eq!(sanitize_error_body("hello"), "hello");
    }
}
