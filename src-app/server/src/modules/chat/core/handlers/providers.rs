// LLM Provider handlers for chat module

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{debug_handler, extract::Query, http::StatusCode, Json};

use crate::{
    common::{ApiResult, AppError, DEFAULT_PAGE_SIZE, PAGINATION_MAX_PER_PAGE},
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
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatUserProvidersQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[debug_handler]
pub async fn get_user_llm_providers(
    auth: RequirePermissions<(ConversationsRead,)>,
    Query(q): Query<ChatUserProvidersQuery>,
) -> ApiResult<Json<GetUserProvidersResponse>> {
    let limit = q
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE as i64)
        .clamp(1, PAGINATION_MAX_PER_PAGE as i64);
    let offset = q.offset.unwrap_or(0).max(0);

    // Get providers accessible to the user based on group assignments
    let providers = Repos
        .user_group_llm_provider
        .get_for_user(auth.user.id, limit, offset)
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
