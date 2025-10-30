// LLM Repository service layer with validation and connectivity testing
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: All validation logic copied exactly from react-test

use crate::common::AppError;
use super::{
    models::{LlmRepository, RepositoryAuthConfig},
    types::{CreateLlmRepositoryRequest, TestRepositoryConnectionRequest, UpdateLlmRepositoryRequest},
};

/// Validate URL format using reqwest URL parser
pub fn validate_url(url: &str) -> Result<(), AppError> {
    if reqwest::Url::parse(url).is_ok() {
        Ok(())
    } else {
        Err(AppError::bad_request("VALIDATION_ERROR", "Invalid URL format"))
    }
}

/// Validate auth type is one of the allowed types
pub fn validate_auth_type(auth_type: &str) -> Result<(), AppError> {
    let valid_auth_types = ["none", "api_key", "basic_auth", "bearer_token"];
    if valid_auth_types.contains(&auth_type) {
        Ok(())
    } else {
        Err(AppError::bad_request("VALIDATION_ERROR", "Invalid authentication type"))
    }
}

/// Validate authentication configuration for create request
/// Ensures all required fields are present based on auth_type
pub fn validate_auth_config_for_create(
    request: &CreateLlmRepositoryRequest,
) -> Result<(), AppError> {
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
/// - 30s timeout
/// - Hugging Face special handling (Bearer vs X-API-Key)
/// - auth_test_api_endpoint support
/// - Only HTTP 200 is success
pub async fn test_repository_connectivity(
    request: &TestRepositoryConnectionRequest,
) -> Result<(), String> {
    // Create a reqwest client with timeout
    let client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));

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

    println!("Testing connection to: {}", test_url);

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
