// AI Provider creation and configuration
//
// This module handles fetching model/provider info and creating provider instances

use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use ai_providers::Provider;

use crate::common::AppError;
use crate::modules::llm_model::repository::LlmModelRepository;
use crate::modules::llm_provider::repository::LlmProviderRepository;

/// Create an AI provider instance from a model ID
///
/// This function:
/// 1. Fetches the model from the database
/// 2. Fetches the associated provider
/// 3. Validates the provider is enabled
/// 4. Creates and configures the provider instance
pub async fn create_provider_from_model_id(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<(Arc<Provider>, String, Uuid, Uuid), AppError> {
    let model_repo = LlmModelRepository::new(pool.clone());
    let provider_repo = LlmProviderRepository::new(pool.clone());

    // Get model information
    let model = model_repo
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Get provider information
    let provider_info = provider_repo
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

    // Get API key and base URL
    let api_key = provider_info.api_key.as_deref().unwrap_or("");
    let base_url = provider_info.base_url.as_deref().unwrap_or_else(|| match provider_type {
        "anthropic" => "https://api.anthropic.com",
        "gemini" => "https://generativelanguage.googleapis.com",
        _ => "https://api.openai.com/v1",
    });

    // Create provider instance
    let provider = Arc::new(
        Provider::new(provider_type, api_key, base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create provider: {}", e)))?,
    );

    // Return provider along with model metadata
    Ok((provider, model.name.clone(), model.id, model.provider_id))
}
