// LLM Repository service layer with validation and connectivity testing
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: All validation logic copied exactly from react-test

use super::{
    models::LlmRepository,
    types::{
        CreateLlmRepositoryRequest, TestRepositoryConnectionRequest, UpdateLlmRepositoryRequest,
    },
};
use crate::common::AppError;

/// Replace any URL userinfo (`https://user:topsecret@host`) with
/// `https://[REDACTED]@host` before logging. Closes
/// 09-llm-repository F-04 (High) on the test-connection path.
fn redact_url_userinfo(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            if !u.username().is_empty() || u.password().is_some() {
                let _ = u.set_username("");
                let _ = u.set_password(None);
                format!("{} (userinfo redacted)", u)
            } else {
                u.to_string()
            }
        }
        Err(_) => "[unparseable URL]".to_string(),
    }
}

/// Validate URL format using reqwest URL parser
pub fn validate_url(url: &str) -> Result<(), AppError> {
    // SSRF-safe validation: reject non-allowlisted schemes (file://, ftp://,
    // git://, gopher://, data:), reject private/loopback/link-local IPs
    // (RFC 1918 + 127/8 + 169.254/16 — AWS IMDS), reject URLs embedding
    // credentials. The previous implementation only checked Url::parse
    // succeeded — that admitted every SSRF flagged by 09-llm-repository
    // F-01 + F-03. PUBLIC_HTTP_OR_HTTPS allows both http and https since
    // self-hosted upstreams may not yet have TLS, but blocks all private
    // address space.
    crate::utils::url_validator::validate_outbound_url(
        url,
        &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
    )
    .map(|_| ())
    .map_err(|e| AppError::bad_request("INVALID_URL", e.to_string()))
}

/// Validate auth type is one of the allowed types
pub fn validate_auth_type(auth_type: &str) -> Result<(), AppError> {
    let valid_auth_types = ["none", "api_key", "basic_auth", "bearer_token"];
    if valid_auth_types.contains(&auth_type) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Invalid authentication type",
        ))
    }
}

/// Validate authentication configuration for create request
/// Max repository name length. Closes 09-llm-repository F-10 (Medium):
/// without this, an admin could store a multi-MB name that the UI
/// renders without escaping → XSS surface; even with escaping the
/// payload is wasteful.
const MAX_REPO_NAME_LEN: usize = 128;

/// Bound + validate the optional auth_test_api_endpoint URL. Closes
/// 09-llm-repository F-08 (Medium): the field was unvalidated
/// free-form text stored in DB, then fetched without scheme/host
/// gating — SSRF surface on test-connection paths. Validates via
/// the shared outbound URL allowlist (no file://, no RFC1918, etc.)
/// when present.
fn validate_test_endpoint(endpoint: &Option<String>) -> Result<(), AppError> {
    if let Some(url) = endpoint {
        if url.trim().is_empty() {
            return Ok(());
        }
        if url.len() > 2048 {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "auth_test_api_endpoint exceeds 2048 chars",
            ));
        }
        crate::utils::url_validator::validate_outbound_url(
            url,
            &crate::utils::url_validator::OutboundUrlPolicy::DEV_LOCAL,
        )
        .map_err(|e| {
            AppError::bad_request(
                "VALIDATION_ERROR",
                format!("auth_test_api_endpoint invalid: {}", e),
            )
        })?;
    }
    Ok(())
}

/// Ensures all required fields are present based on auth_type
pub fn validate_auth_config_for_create(
    request: &CreateLlmRepositoryRequest,
) -> Result<(), AppError> {
    // Bound the repository name (09-llm-repository F-10).
    if request.name.len() > MAX_REPO_NAME_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("Repository name exceeds {} chars", MAX_REPO_NAME_LEN),
        ));
    }
    // Validate auth_test_api_endpoint when set (09-llm-repository F-08).
    if let Some(ac) = &request.auth_config {
        validate_test_endpoint(&ac.auth_test_api_endpoint)?;
    }

    // If auth_type is not "none", auth_config must be provided
    if request.auth_type != "none" && request.auth_config.is_none() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Authentication configuration is required for non-none authentication types",
        ));
    }

    // Validate required fields based on auth type
    if let Some(auth_config) = &request.auth_config {
        match request.auth_type.as_str() {
            "api_key" => {
                if auth_config.api_key.is_none()
                    || auth_config.api_key.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "API key is required for api_key authentication",
                    ));
                }
            }
            "basic_auth" => {
                if auth_config.username.is_none()
                    || auth_config.username.as_ref().unwrap().trim().is_empty()
                    || auth_config.password.is_none()
                    || auth_config.password.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Username and password are required for basic_auth authentication",
                    ));
                }
            }
            "bearer_token" => {
                if auth_config.token.is_none()
                    || auth_config.token.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Bearer token is required for bearer_token authentication",
                    ));
                }
            }
            _ => {} // "none" requires no additional validation
        }
    }

    Ok(())
}

/// Validate authentication configuration for update request
/// Merges current repository auth with new auth fields and validates the result
pub fn validate_auth_config_for_update(
    current_repository: &LlmRepository,
    request: &UpdateLlmRepositoryRequest,
) -> Result<(), AppError> {
    // Mirror create-time bounds (09-llm-repository F-08/F-10).
    if let Some(name) = &request.name
        && name.len() > MAX_REPO_NAME_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("Repository name exceeds {} chars", MAX_REPO_NAME_LEN),
            ));
        }
    if let Some(ac) = &request.auth_config {
        validate_test_endpoint(&ac.auth_test_api_endpoint)?;
    }

    // Determine which auth_type to use (new or current)
    let auth_type = request
        .auth_type
        .as_ref()
        .unwrap_or(&current_repository.auth_type);

    // If auth_config is being updated, validate it
    if let Some(new_auth_config) = &request.auth_config {
        match auth_type.as_str() {
            "api_key" => {
                // Check if new config has api_key
                if new_auth_config.api_key.is_none()
                    || new_auth_config.api_key.as_ref().unwrap().trim().is_empty()
                {
                    // Check if current repository has api_key
                    if current_repository.auth_config.api_key.is_none()
                        || current_repository
                            .auth_config
                            .api_key
                            .as_ref()
                            .unwrap()
                            .trim()
                            .is_empty()
                    {
                        return Err(AppError::bad_request(
                            "VALIDATION_ERROR",
                            "API key is required for api_key authentication",
                        ));
                    }
                }
            }
            "basic_auth" => {
                // Merge username and password from new and current
                let username = new_auth_config
                    .username
                    .as_ref()
                    .or(current_repository.auth_config.username.as_ref());
                let password = new_auth_config
                    .password
                    .as_ref()
                    .or(current_repository.auth_config.password.as_ref());

                if username.is_none()
                    || username.unwrap().trim().is_empty()
                    || password.is_none()
                    || password.unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Username and password are required for basic_auth authentication",
                    ));
                }
            }
            "bearer_token" => {
                // Merge token from new and current
                let token = new_auth_config
                    .token
                    .as_ref()
                    .or(current_repository.auth_config.token.as_ref());

                if token.is_none() || token.unwrap().trim().is_empty() {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Bearer token is required for bearer_token authentication",
                    ));
                }
            }
            _ => {} // "none" requires no additional validation
        }
    }

    Ok(())
}

/// Test repository connectivity
/// Copied exactly from react-test - includes all business logic:
/// - 10s timeout (reduced from 30s for faster failure on invalid credentials)
/// - Hugging Face special handling (Bearer vs X-API-Key)
/// - auth_test_api_endpoint support
/// - Only HTTP 200 is success
///
/// NOTE: this validates REST-API reachability/credentials against
/// `auth_test_api_endpoint`, NOT the git-clone Basic-over-HTTPS path that the
/// actual model download uses (see the credential callbacks in
/// `utils/git/service.rs`). For `auth_type == "basic_auth"` the two wire formats
/// differ, so a green "Test connection" does not by itself guarantee the clone
/// will authenticate.
pub async fn test_repository_connectivity(
    request: &TestRepositoryConnectionRequest,
) -> Result<(), String> {
    // Create a reqwest client with timeout (10s for faster feedback).
    // A non-empty User-Agent is REQUIRED: GitHub's REST API (the seeded GitHub
    // test endpoint https://api.github.com/user) rejects any UA-less request with
    // 403 Forbidden *before* checking the token, so a valid token would otherwise
    // fail the connection test with a misleading 403.
    let client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("ziee/", env!("CARGO_PKG_VERSION")));

    let client = client_builder
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Determine the test URL - use auth_test_api_endpoint if provided, otherwise use the main URL
    let test_url = if let Some(auth_config) = &request.auth_config {
        if let Some(ref test_endpoint) = auth_config.auth_test_api_endpoint {
            if !test_endpoint.trim().is_empty() {
                test_endpoint
            } else {
                &request.url
            }
        } else {
            &request.url
        }
    } else {
        &request.url
    };

    // Build the request with authentication
    let mut req_builder = client.get(test_url);

    tracing::info!("Testing connection to: {}", redact_url_userinfo(test_url));

    if let Some(auth_config) = &request.auth_config {
        match request.auth_type.as_str() {
            "api_key" => {
                if let Some(api_key) = &auth_config.api_key {
                    // For Hugging Face, use Bearer token format
                    if request.url.contains("huggingface.co") {
                        req_builder =
                            req_builder.header("Authorization", format!("Bearer {}", api_key));
                    } else {
                        // For other APIs, use X-API-Key header (common pattern)
                        req_builder = req_builder.header("X-API-Key", api_key);
                    }
                }
            }
            "basic_auth" => {
                if let (Some(username), Some(password)) =
                    (&auth_config.username, &auth_config.password)
                {
                    req_builder = req_builder.basic_auth(username, Some(password));
                }
            }
            "bearer_token" => {
                if let Some(token) = &auth_config.token {
                    req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
                }
            }
            _ => {} // "none" - no authentication
        }
    }

    // Make the request
    match req_builder.send().await {
        Ok(response) => {
            let status = response.status();
            if status == 200 {
                // Only consider HTTP 200 as successful
                Ok(())
            } else {
                Err(format!("HTTP request failed with status: {}", status))
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("timeout") {
                Err("Connection timed out".to_string())
            } else if error_msg.contains("dns") {
                Err(format!("DNS resolution failed: {}", e))
            } else if error_msg.contains("connection") {
                Err(format!("Connection failed: {}", e))
            } else {
                Err(format!("Network request failed: {}", e))
            }
        }
    }
}
