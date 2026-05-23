// LLM Provider service layer with validation
// Similar to LLM Repository service but for provider management

use super::types::{CreateLlmProviderRequest, UpdateLlmProviderRequest};
use crate::common::AppError;

/// Validate provider type is one of the allowed types
pub fn validate_provider_type(provider_type: &str) -> Result<(), AppError> {
    let valid_types = [
        "local",
        "openai",
        "anthropic",
        "groq",
        "gemini",
        "mistral",
        "deepseek",
        "huggingface",
        "custom",
    ];
    if valid_types.contains(&provider_type) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Invalid provider type",
        ))
    }
}

/// Validate base URL format if provided.
///
/// SSRF-safe: rejects non-HTTP schemes (file://, ftp://, git://, gopher://,
/// data:), rejects private/loopback/link-local IPs (RFC 1918 + 127/8 +
/// 169.254/16 — AWS IMDS), and rejects URLs embedding credentials. The
/// previous implementation only checked Url::parse succeeded — that
/// admitted every SSRF the audit flagged in 06-llm-provider F-03.
///
/// PUBLIC_HTTP_OR_HTTPS allows both http and https since self-hosted
/// OpenAI-compatible providers may not yet have TLS.
pub fn validate_base_url(base_url: &Option<String>) -> Result<(), AppError> {
    if let Some(url) = base_url {
        if !url.is_empty() {
            crate::utils::url_validator::validate_outbound_url(
                url,
                &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
            )
            .map_err(|e| AppError::bad_request("INVALID_BASE_URL", e.to_string()))?;
        }
    }
    Ok(())
}

/// Validate that required fields are present for enabled providers
pub fn validate_create_request(request: &CreateLlmProviderRequest) -> Result<(), AppError> {
    // Validate name is not empty
    if request.name.trim().is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Provider name cannot be empty",
        ));
    }

    // Validate provider type
    validate_provider_type(&request.provider_type)?;

    // Validate base URL if provided
    validate_base_url(&request.base_url)?;

    // If enabling the provider, ensure required fields are present
    if request.enabled.unwrap_or(false) {
        // For remote providers (not local), API key is usually required
        if request.provider_type != "local" && request.provider_type != "custom" {
            if request.api_key.is_none() || request.api_key.as_ref().unwrap().trim().is_empty() {
                return Err(AppError::bad_request(
                    "VALIDATION_ERROR",
                    "API key is required for enabled remote providers",
                ));
            }
        }
    }

    Ok(())
}

/// Validate update request
pub fn validate_update_request(request: &UpdateLlmProviderRequest) -> Result<(), AppError> {
    // Validate name is not empty if being updated
    if let Some(name) = &request.name {
        if name.trim().is_empty() {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "Provider name cannot be empty",
            ));
        }
    }

    // Validate base URL if being updated
    validate_base_url(&request.base_url)?;

    Ok(())
}
