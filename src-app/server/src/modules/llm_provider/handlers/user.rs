// User-facing LLM provider handlers

use aide::transform::TransformOperation;
use axum::{debug_handler, extract::Path, http::StatusCode, Json};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        permissions::{extractors::RequirePermissions, with_permission},
        user::permissions::{ProfileEdit, ProfileRead},
    },
};

use super::super::{
    permissions::UserLlmProvidersRead,
    types::{
        GetUserProvidersResponse, ProviderWithModels, SaveUserApiKeyRequest, UserApiKeyListResponse,
    },
};

/// Get LLM providers accessible to the authenticated user
#[debug_handler]
pub async fn get_user_llm_providers(
    auth: RequirePermissions<(UserLlmProvidersRead,)>,
) -> ApiResult<Json<GetUserProvidersResponse>> {
    let user_id = auth.user.id;

    let providers = Repos
        .user_group_llm_provider
        .get_for_user(user_id)
        .await
        .map_err(AppError::from)?;

    let mut providers_with_models = Vec::new();

    for provider in providers {
        let all_models = Repos
            .llm_model
            .list_by_provider(provider.id)
            .await?;

        let enabled_models: Vec<_> = all_models.into_iter().filter(|m| m.enabled).collect();

        let system_key_configured = provider.api_key
            .as_deref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);
        let user_key_configured = Repos.user_key.has_key(user_id, provider.id).await?;
        let api_key_configured = system_key_configured || user_key_configured;

        providers_with_models.push(ProviderWithModels {
            provider,
            llm_models: enabled_models,
            api_key_configured,
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
    with_permission::<(UserLlmProvidersRead,)>(op)
        .id("LlmProvider.getUserLlmProviders")
        .tag("LlmProvider")
        .summary("Get user's accessible LLM providers")
        .description(
            "Returns all enabled LLM providers assigned to the user's groups, \
             with enabled models and API key configuration status.",
        )
        .response::<200, Json<GetUserProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// List user's stored API keys (masked)
#[debug_handler]
pub async fn list_user_api_keys(
    auth: RequirePermissions<(ProfileRead,)>,
) -> ApiResult<Json<UserApiKeyListResponse>> {
    let keys = Repos.user_key.list_for_user(auth.user.id).await?;
    Ok((StatusCode::OK, Json(UserApiKeyListResponse { keys })))
}

pub fn list_user_api_keys_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("LlmProvider.listUserApiKeys")
        .tag("LlmProvider")
        .summary("List user's provider API keys (masked)")
        .response::<200, Json<UserApiKeyListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Save or update a user API key for a provider
#[debug_handler]
pub async fn save_user_api_key(
    auth: RequirePermissions<(ProfileEdit,)>,
    Json(request): Json<SaveUserApiKeyRequest>,
) -> ApiResult<()> {
    let key = request.api_key.trim().to_string();

    if key.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "API key cannot be empty").into());
    }
    if key.len() > 500 {
        return Err(AppError::bad_request("VALIDATION_ERROR", "API key too long").into());
    }
    if key.bytes().any(|b| b < 0x20 && b != b'\t') {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "API key contains invalid characters").into(),
        );
    }

    Repos
        .user_key
        .upsert(auth.user.id, request.provider_id, &key)
        .await?;

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn save_user_api_key_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileEdit,)>(op)
        .id("LlmProvider.saveUserApiKey")
        .tag("LlmProvider")
        .summary("Save or update user API key for a provider")
        .response::<204, ()>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Delete a user API key for a provider
#[debug_handler]
pub async fn delete_user_api_key(
    auth: RequirePermissions<(ProfileEdit,)>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<()> {
    Repos
        .user_key
        .delete(auth.user.id, provider_id)
        .await?;

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_user_api_key_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileEdit,)>(op)
        .id("LlmProvider.deleteUserApiKey")
        .tag("LlmProvider")
        .summary("Delete user API key for a provider")
        .response::<204, ()>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
