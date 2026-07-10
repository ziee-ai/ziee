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
fn sanitize_error_body(body: &str) -> String {
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

    /// Creates error from an Anthropic error event. Both fields are untrusted
    /// provider output (HTTP error body or SSE `error` event), so each is bounded +
    /// newline-collapsed via `sanitize_error_body` before being embedded.
    pub fn from_anthropic_error(error_type: &str, message: &str) -> Self {
        let error_type = sanitize_error_body(error_type);
        let message = sanitize_error_body(message);
        match error_type.as_str() {
            "overloaded_error" => Self::rate_limit(format!("Overloaded: {}", message)),
            "rate_limit_error" => Self::rate_limit(message),
            "authentication_error" => Self::auth(message),
            "invalid_request_error" => Self::invalid_request(message),
            "permission_error" => Self::auth(message),
            _ => Self::provider(format!("Anthropic {}: {}", error_type, message)),
        }
    }

    /// Classify by HTTP status (reliable across providers) using an
    /// already-sanitized `message` as the text. The status is the authoritative
    /// signal (401/403→auth, 429→rate-limit, 400/404→invalid, 408/504→timeout).
    fn from_status_with_message(status: u16, message: String) -> Self {
        match status {
            401 | 403 => Self::auth(message),
            429 => Self::rate_limit(message),
            400 | 404 => Self::invalid_request(message),
            408 | 504 => Self::timeout(message),
            _ => Self::provider(message),
        }
    }

    /// Build a clean, typed error from an Anthropic HTTP error body. The HTTP
    /// status drives the variant (so a 404 stays invalid-request, a 429 stays
    /// rate-limit); the parsed `{"error":{type,message}}` message is the text.
    /// The response side of the adapter maps each provider's error wire shape
    /// into the common `ProviderError` — no reactive self-heal.
    pub fn from_anthropic_http(status: u16, body: &str) -> Self {
        let message = parse_anthropic_error(body)
            .map(|(_, m)| m)
            .unwrap_or_else(|| body.to_string());
        Self::from_status_with_message(status, sanitize_error_body(&message))
    }

    /// Build a clean, typed error from an OpenAI HTTP error body
    /// (`{"error":{"message","type","param","code"}}`). Status-driven variant +
    /// the parsed message.
    pub fn from_openai_http(status: u16, body: &str) -> Self {
        let message = parse_openai_error(body)
            .map(|e| e.message)
            .unwrap_or_else(|| body.to_string());
        Self::from_status_with_message(status, sanitize_error_body(&message))
    }

    /// Build a clean, typed error from a Gemini HTTP error body
    /// (`{"error":{"status","message"}}`). Status-driven variant + the parsed
    /// message (the RPC `status` string is informational only).
    pub fn from_gemini_http(status: u16, body: &str) -> Self {
        let message = parse_gemini_error(body)
            .map(|(_, m)| m)
            .unwrap_or_else(|| body.to_string());
        Self::from_status_with_message(status, sanitize_error_body(&message))
    }

    /// A Gemini in-stream `promptFeedback.blockReason` (untrusted, free-string)
    /// as a typed error, sanitized/bounded like every other provider string.
    pub fn gemini_prompt_blocked(block_reason: &str) -> Self {
        Self::provider(format!("Prompt blocked: {}", sanitize_error_body(block_reason)))
    }
}

/// A parsed OpenAI error envelope. `param`/`code` are retained for callers that
/// want to reason about which field was rejected (surfaced as a clean message;
/// there is no reactive self-heal).
pub(crate) struct OpenAiError {
    pub message: String,
    #[allow(dead_code)]
    pub code: Option<String>,
    #[allow(dead_code)]
    pub param: Option<String>,
}

/// Parse `{"error":{"message","type","param","code"}}` (OpenAI-compatible).
pub(crate) fn parse_openai_error(body: &str) -> Option<OpenAiError> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    let message = err.get("message")?.as_str()?.to_string();
    Some(OpenAiError {
        message,
        code: err.get("code").and_then(|c| c.as_str()).map(str::to_string),
        param: err.get("param").and_then(|p| p.as_str()).map(str::to_string),
    })
}

/// Parse an Anthropic error envelope `{"error":{"type","message"}}` into its
/// `(type, message)` pair; `None` when the body isn't the expected shape.
pub(crate) fn parse_anthropic_error(body: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    let ty = err.get("type")?.as_str()?.to_string();
    let msg = err.get("message")?.as_str()?.to_string();
    Some((ty, msg))
}

/// Parse a Gemini error envelope `{"error":{"status","message"}}` into its
/// `(status, message)` pair; `None` when the body isn't the expected shape.
pub(crate) fn parse_gemini_error(body: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    let status = err
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let msg = err.get("message")?.as_str()?.to_string();
    Some((status, msg))
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

    // TEST-11: typed per-provider error parsers → the common ProviderError.
    // The HTTP status drives the variant; the parsed message is preserved (and a
    // clean message — not the raw JSON blob — proves the typed parse ran).
    #[test]
    fn openai_error_maps_to_typed_variant() {
        let body = r#"{"error":{"message":"Unsupported parameter: 'temperature'","type":"invalid_request_error","param":"temperature","code":"unsupported_parameter"}}"#;
        let parsed = parse_openai_error(body).expect("parses");
        assert_eq!(parsed.param.as_deref(), Some("temperature"));
        assert_eq!(parsed.code.as_deref(), Some("unsupported_parameter"));
        match ProviderError::from_openai_http(400, body) {
            ProviderError::InvalidRequest(m) => {
                assert!(m.contains("Unsupported parameter"), "clean message preserved");
                assert!(!m.contains('{'), "raw JSON blob must not leak");
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        // Status is authoritative regardless of the envelope contents.
        assert!(matches!(
            ProviderError::from_openai_http(429, body),
            ProviderError::RateLimit(_)
        ));
        // Unparseable body still classifies by status (raw body as text).
        assert!(matches!(
            ProviderError::from_openai_http(401, "not json"),
            ProviderError::Authentication(_)
        ));
    }

    #[test]
    fn anthropic_error_maps_to_typed_variant() {
        let body = r#"{"error":{"type":"not_found_error","message":"model not found"}}"#;
        let (ty, msg) = parse_anthropic_error(body).expect("parses");
        assert_eq!(ty, "not_found_error");
        assert_eq!(msg, "model not found");
        // A 404 with a non-400 `type` still classifies by HTTP status (was a
        // regression when it routed everything through the type mapping).
        match ProviderError::from_anthropic_http(404, body) {
            ProviderError::InvalidRequest(m) => assert!(m.contains("model not found")),
            other => panic!("expected InvalidRequest for 404, got {other:?}"),
        }
    }

    #[test]
    fn gemini_error_maps_status_to_variant() {
        let body = r#"{"error":{"code":400,"status":"INVALID_ARGUMENT","message":"bad top_k"}}"#;
        let (status, msg) = parse_gemini_error(body).expect("parses");
        assert_eq!(status, "INVALID_ARGUMENT");
        assert_eq!(msg, "bad top_k");
        match ProviderError::from_gemini_http(400, body) {
            ProviderError::InvalidRequest(m) => assert!(m.contains("bad top_k")),
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        // A 429 whose envelope `status` is empty/non-standard still maps to
        // RateLimit by HTTP status (was downgraded to generic provider before).
        let empty_status = r#"{"error":{"message":"quota"}}"#;
        assert!(matches!(
            ProviderError::from_gemini_http(429, empty_status),
            ProviderError::RateLimit(_)
        ));
        let perm = r#"{"error":{"status":"PERMISSION_DENIED","message":"no"}}"#;
        assert!(matches!(
            ProviderError::from_gemini_http(403, perm),
            ProviderError::Authentication(_)
        ));
    }

    #[test]
    fn gemini_prompt_blocked_is_sanitized() {
        // CR/LF-laden untrusted block reason must be collapsed (anti log-forging).
        let e = ProviderError::gemini_prompt_blocked("SAFETY\n injected: line");
        let s = e.to_string();
        assert!(!s.contains('\n') && !s.contains('\r'));
        assert!(s.contains("Prompt blocked"));
    }
}
