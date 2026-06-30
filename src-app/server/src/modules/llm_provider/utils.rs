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
/// data:) and rejects private / link-local IPs (RFC 1918, 169.254/16
/// — AWS IMDS, ULA, CGNAT). The previous implementation only checked
/// Url::parse succeeded — that admitted every SSRF the audit flagged in
/// 06-llm-provider F-03.
///
/// Uses DEV_LOCAL (allow_localhost=true, allow_private=false) instead of
/// the stricter PUBLIC_HTTP_OR_HTTPS because local LLM providers
/// (e.g., llama.cpp, mistralrs running on http://localhost:1234/v1) are
/// a legitimate first-class use case for this product. Localhost
/// providers are an admin-only feature anyway (requires
/// `llm_providers::create`), so the admin-can-probe-localhost risk is
/// already gated by trust.
pub fn validate_base_url(base_url: &Option<String>) -> Result<(), AppError> {
    if let Some(url) = base_url
        && !url.is_empty() {
            crate::utils::url_validator::validate_outbound_url(
                url,
                &crate::utils::url_validator::OutboundUrlPolicy::DEV_LOCAL,
            )
            .map_err(|e| AppError::bad_request("INVALID_BASE_URL", e.to_string()))?;
        }
    Ok(())
}

/// Maximum lengths for provider fields. Closes 06-llm-provider F-09
/// (Medium): without these, an admin (or a compromised admin account)
/// could store multi-MB strings that bloat the DB row, slow every
/// list query, and inflate response payloads.
const MAX_NAME_LEN: usize = 128;
const MAX_BASE_URL_LEN: usize = 2048;
const MAX_API_KEY_LEN: usize = 4096;

/// Reject control characters in a string-typed field. Closes
/// 06-llm-provider F-12 (Medium): `\n`/`\r`/`\0` in a provider name
/// could break log lines and JSON rendering downstream.
fn reject_control_chars(label: &str, value: &str) -> Result<(), AppError> {
    if value.chars().any(|c| c.is_control()) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("{} cannot contain control characters", label),
        ));
    }
    Ok(())
}

/// Validate that required fields are present for enabled providers
pub fn validate_create_request(request: &CreateLlmProviderRequest) -> Result<(), AppError> {
    // Validate name is not empty + bounded + free of control chars
    let trimmed_name = request.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Provider name cannot be empty",
        ));
    }
    if request.name.len() > MAX_NAME_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("Provider name exceeds {} chars", MAX_NAME_LEN),
        ));
    }
    reject_control_chars("Provider name", &request.name)?;

    // Validate provider type
    validate_provider_type(&request.provider_type)?;

    // Validate base URL if provided
    if let Some(base_url) = &request.base_url
        && base_url.len() > MAX_BASE_URL_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("base_url exceeds {} chars", MAX_BASE_URL_LEN),
            ));
        }
    validate_base_url(&request.base_url)?;

    // Bound api_key length to prevent multi-MB rows on encrypted columns.
    if let Some(api_key) = &request.api_key
        && api_key.len() > MAX_API_KEY_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("api_key exceeds {} chars", MAX_API_KEY_LEN),
            ));
        }

    // NOTE: an enabled remote provider with no API key is NOT a hard error.
    // Onboarding deliberately creates a keyless remote provider so the admin
    // (or user) can paste their own key later. The create handler coerces such
    // a provider to `enabled=false` (see `remote_provider_needs_key` +
    // `create_provider`) instead of rejecting it — if the admin supplied the
    // key it stays enabled; if not, it is created disabled.

    Ok(())
}

/// True when a request describes a *remote* provider (not `local`/`custom`)
/// that is being enabled without an API key. Such a provider must be created
/// disabled — it cannot serve traffic until a key is supplied.
pub fn remote_provider_needs_key(
    provider_type: &str,
    enabled: bool,
    api_key: Option<&String>,
) -> bool {
    enabled
        && provider_type != "local"
        && provider_type != "custom"
        && api_key.map(|k| k.trim().is_empty()).unwrap_or(true)
}

/// Validate update request
pub fn validate_update_request(request: &UpdateLlmProviderRequest) -> Result<(), AppError> {
    // Validate name if being updated
    if let Some(name) = &request.name {
        if name.trim().is_empty() {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "Provider name cannot be empty",
            ));
        }
        if name.len() > MAX_NAME_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("Provider name exceeds {} chars", MAX_NAME_LEN),
            ));
        }
        reject_control_chars("Provider name", name)?;
    }

    // Validate base URL if being updated
    if let Some(base_url) = &request.base_url
        && base_url.len() > MAX_BASE_URL_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("base_url exceeds {} chars", MAX_BASE_URL_LEN),
            ));
        }
    validate_base_url(&request.base_url)?;

    // Bound api_key length on update too.
    if let Some(api_key) = &request.api_key
        && api_key.len() > MAX_API_KEY_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!("api_key exceeds {} chars", MAX_API_KEY_LEN),
            ));
        }

    Ok(())
}
