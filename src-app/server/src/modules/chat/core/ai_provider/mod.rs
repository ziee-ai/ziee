// AI Provider creation and configuration
//
// This module handles fetching model/provider info and creating provider instances

use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use ai_providers::Provider;

use crate::common::AppError;
use crate::core::Repos;

/// Create an AI provider instance from a model ID
///
/// This function:
/// 1. Fetches the model from the database
/// 2. Fetches the associated provider
/// 3. Validates the provider is enabled
/// 4. Creates and configures the provider instance
/// 5. Uses user's personal API key if available, falls back to system key
pub async fn create_provider_from_model_id(
    _pool: &PgPool,
    model_id: Uuid,
    user_id: Uuid,
) -> Result<(Arc<Provider>, String, Uuid, Uuid), AppError> {
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

    // Resolve API key: user key → system key → empty string
    let user_api_key = Repos.user_key
        .get(user_id, provider_info.id)
        .await?;

    let api_key = user_api_key
        .or(provider_info.api_key)
        .unwrap_or_default();

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

    // Return provider along with model metadata
    Ok((provider, model.name.clone(), model.id, model.provider_id))
}
