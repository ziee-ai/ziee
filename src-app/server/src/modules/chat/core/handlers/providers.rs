// LLM Provider handlers for chat module

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{debug_handler, http::StatusCode, Json};

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            permissions::ConversationsRead, types::GetUserProvidersResponse,
            types::ProviderWithModels,
        },
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

/// Get LLM providers accessible to the authenticated user
///
/// Returns all enabled LLM providers assigned to the user's active groups,
/// with their enabled and active models included.
#[debug_handler]
pub async fn get_user_llm_providers(
    auth: RequirePermissions<(ConversationsRead,)>,
) -> ApiResult<Json<GetUserProvidersResponse>> {
    // Get providers accessible to the user based on group assignments
    let providers = Repos
        .llm_provider
        .get_for_user(auth.user.id)
        .await
        .map_err(AppError::from)?;

    // For each provider, fetch its models and filter to enabled+active
    let mut providers_with_models = Vec::new();

    for provider in providers {
        // Get all models for this provider
        let all_models = Repos
            .llm_model
            .list_by_provider(provider.id)
            .await?;

        // Filter to only enabled models
        let enabled_models: Vec<_> = all_models
            .into_iter()
            .filter(|model| model.enabled)
            .collect();

        providers_with_models.push(ProviderWithModels {
            provider,
            llm_models: enabled_models,
        });
    }

    Ok((
        StatusCode::OK,
        Json(GetUserProvidersResponse {
            providers: providers_with_models,
        }),
    ))
}

pub fn get_user_llm_providers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Chat.getUserLlmProviders")
        .tag("Chat")
        .summary("Get user's accessible LLM providers")
        .description(
            "Returns all enabled LLM providers assigned to the user's active groups, \
             with their enabled and active models included inline.",
        )
        .response::<200, Json<GetUserProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
