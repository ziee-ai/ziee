// AI Provider creation and configuration
//
// This module handles fetching model/provider info and creating provider instances

use std::sync::Arc;
use uuid::Uuid;

use ai_providers::Provider;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::llm_provider::UserKeyRepository;

/// Resolve which API key to use for a given user + provider.
///
/// Priority chain:
///   1. The user's personal key from `user_llm_provider_api_keys` (if set).
///   2. The admin-configured system key on `llm_providers.api_key` (if set).
///   3. An empty string (some provider types — `local`, `custom` — accept this).
///
/// Takes the `UserKeyRepository` explicitly (rather than reaching for the
/// global `Repos`) so it can be exercised from integration tests, which run
/// in a separate process from the test server and don't have `Repos`
/// initialised.
pub async fn resolve_api_key_for_user(
    user_key_repo: &UserKeyRepository,
    user_id: Uuid,
    provider_id: Uuid,
    system_key: Option<String>,
) -> Result<String, AppError> {
    let user_key = user_key_repo.get(user_id, provider_id).await?;
    Ok(user_key.or(system_key).unwrap_or_default())
}

/// Create an AI provider instance from a model ID
///
/// This function:
/// 1. Fetches the model from the database
/// 2. Fetches the associated provider
/// 3. Validates the provider is enabled
/// 4. Resolves the API key (user's personal key wins, falls back to system)
/// 5. Creates and configures the provider instance
pub async fn create_provider_from_model_id(
    model_id: Uuid,
    user_id: Uuid,
) -> Result<
    (
        Arc<Provider>,
        String,
        Uuid,
        Uuid,
        crate::modules::llm_model::models::ModelParameters,
    ),
    AppError,
> {
    // Get model information
    let model = Repos.llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Get provider information
    let provider_info = Repos.llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    // Check if provider is enabled
    if !provider_info.enabled {
        return Err(AppError::bad_request(
            "PROVIDER_DISABLED",
            "The provider for this model is currently disabled",
        ));
    }

    // Map provider type to ai_providers format
    // anthropic and gemini map directly, everything else uses OpenAI-compatible API
    let provider_type = match provider_info.provider_type.as_str() {
        "anthropic" => "anthropic",
        "gemini" => "gemini",
        _ => "openai", // openai, groq, mistral, deepseek, custom, huggingface all use OpenAI-compatible API
    };

    // Resolve API key. Local providers authenticate via the server-minted proxy
    // token stored as the system key — never a user-supplied key (the proxy
    // would reject it). Use the system key directly and ignore any (possibly
    // stale, pre-guard) user key. Every other provider type uses the
    // user-key → system-key → empty chain.
    let api_key = if provider_info.provider_type == "local" {
        provider_info.api_key.clone().unwrap_or_default()
    } else {
        resolve_api_key_for_user(
            &Repos.user_key,
            user_id,
            provider_info.id,
            provider_info.api_key.clone(),
        )
        .await?
    };

    let base_url = provider_info
        .base_url
        .as_deref()
        .ok_or_else(|| AppError::internal_error(
            format!("Provider '{}' has no base_url configured", provider_info.name)
        ))?;

    // Create provider instance
    let provider = Arc::new(
        Provider::new(provider_type, &api_key, base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create provider: {}", e)))?,
    );

    // Return provider along with model metadata + generation parameters.
    Ok((
        provider,
        model.name.clone(),
        model.id,
        model.provider_id,
        model.parameters.clone(),
    ))
}
