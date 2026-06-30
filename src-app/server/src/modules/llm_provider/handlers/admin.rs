// LLM Provider handlers

use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    core::{events::EventBus, repository::Repos},
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

use super::super::{
    events::LlmProviderEvent,
    models::LlmProvider,
    permissions::*,
    types::{
        CreateLlmProviderRequest, CreateLlmProviderResponse, LlmProviderListResponse,
        RotateProxyTokenResponse, UpdateLlmProviderRequest,
    },
    utils,
};

// =====================================================
// Provider CRUD Handlers
// =====================================================

/// List all LLM providers (requires llm_providers::read permission)
#[debug_handler]
pub async fn list_providers(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<LlmProviderListResponse>> {
    // Get all providers
    let all_providers = Repos.llm_provider.list().await.map_err(|e| {
        tracing::error!("Failed to get providers: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Calculate pagination
    let total = all_providers.len() as i64;
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(all_providers.len());

    let paginated_providers = if start < all_providers.len() {
        all_providers[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok((
        StatusCode::OK,
        Json(LlmProviderListResponse {
            providers: paginated_providers,
            total,
            page: params.page,
            per_page: params.per_page,
        }),
    ))
}

pub fn list_providers_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.list")
        .tag("LLM Providers")
        .summary("List all LLM providers with pagination")
        .response::<200, Json<LlmProviderListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM provider by ID (requires llm_providers::read permission)
#[debug_handler]
pub async fn get_provider(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<LlmProvider>> {
    let provider = Repos.llm_provider
        .get_by_id(provider_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    Ok((StatusCode::OK, Json(provider)))
}

pub fn get_provider_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.get")
        .tag("LLM Providers")
        .summary("Get LLM provider by ID")
        .response::<200, Json<LlmProvider>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Provider not found"))
}

/// Create a new LLM provider (requires llm_providers::create permission)
#[debug_handler]
pub async fn create_provider(
    _auth: RequirePermissions<(LlmProvidersCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(mut request): Json<CreateLlmProviderRequest>,
) -> ApiResult<Json<CreateLlmProviderResponse>> {
    // Validate request
    utils::validate_create_request(&request)?;

    // Local-aware enrichment: P1.f wires server-side defaults so the
    // admin only fills the Name field for a local provider.
    //  - base_url stays NULL in the DB; the repository's read seam
    //    injects the live URL from server config + LOCAL_PROXY_PATH
    //    on every read.
    //  - api_key is auto-minted as a 32-byte URL-safe random token.
    //    We return it once in the response body.
    //  - enabled defaults to true (no extra admin click before first chat).
    let mut plaintext_token: Option<String> = None;
    if request.provider_type == "local" {
        // Force-clear caller-supplied base_url + api_key — they're
        // server-managed for local providers and would be ignored
        // anyway at read time.
        request.base_url = None;
        let token = crate::modules::llm_local_runtime::proxy::generate_proxy_token();
        request.api_key = Some(token.clone());
        plaintext_token = Some(token);
        request.enabled.get_or_insert(true);
    }

    // A remote provider enabled without an API key cannot serve traffic, but
    // it is not an error (onboarding creates exactly this so a key can be
    // pasted later). Coerce it to disabled rather than rejecting the create —
    // if the admin DID supply a key it stays enabled.
    if utils::remote_provider_needs_key(
        &request.provider_type,
        request.enabled.unwrap_or(false),
        request.api_key.as_ref(),
    ) {
        request.enabled = Some(false);
    }

    // Create provider
    let provider = Repos.llm_provider.create(request).await.map_err(|e| {
        tracing::error!("Failed to create provider: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Populate the proxy token cache so the proxy front door
    // recognizes the new token immediately (no wait for next boot
    // reseed).
    if provider.provider_type == "local" {
        if let Some(t) = plaintext_token.as_deref() {
            crate::modules::llm_local_runtime::proxy::insert_token(t, provider.id).await;
        }
    }

    // Emit event
    event_bus.emit_async(LlmProviderEvent::created(provider.clone()).into());

    sync_publish(
        SyncEntity::LlmProvider,
        SyncAction::Create,
        provider.id,
        Audience::perm::<LlmProvidersRead>(),
        origin.0,
    );
    sync_publish(
        SyncEntity::UserLlmProvider,
        SyncAction::Create,
        provider.id,
        Audience::perm::<UserLlmProvidersRead>(),
        origin.0,
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateLlmProviderResponse {
            provider,
            plaintext_api_key: plaintext_token,
        }),
    ))
}

pub fn create_provider_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersCreate,)>(op)
        .id("LlmProvider.create")
        .tag("LLM Providers")
        .summary("Create a new LLM provider")
        .description(concat!(
            "For provider_type = 'local', base_url + api_key are server-",
            "derived: the URL is computed from server config + ",
            "LOCAL_PROXY_PATH on read, and the api_key is auto-minted ",
            "(returned once as plaintext_api_key on this response)."
        ))
        .response::<201, Json<CreateLlmProviderResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Rotate the PROXY_TOKEN on a local provider. Returns the new
/// plaintext token; the old token's cache entry is invalidated
/// AFTER the new one is inserted so in-flight requests using the
/// old token can finish.
#[debug_handler]
pub async fn rotate_proxy_token(
    _auth: RequirePermissions<(LlmProvidersEdit,)>,
    Path(provider_id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<Json<RotateProxyTokenResponse>> {
    // Confirm the provider exists and is local.
    let existing = Repos
        .llm_provider
        .get_by_id(provider_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;
    if existing.provider_type != "local" {
        return Err(AppError::bad_request(
            "PROVIDER_NOT_LOCAL",
            "Token rotation only applies to local providers",
        )
        .into());
    }

    let old_token = existing.api_key.clone();
    let new_token = crate::modules::llm_local_runtime::proxy::generate_proxy_token();

    let updated = Repos
        .llm_provider
        .update(
            provider_id,
            UpdateLlmProviderRequest {
                api_key: Some(new_token.clone()),
                name: None,
                enabled: None,
                base_url: None,
                proxy_settings: None,
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to rotate token for {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    // Insert new before removing old — closes the window where neither
    // token would validate.
    crate::modules::llm_local_runtime::proxy::insert_token(&new_token, provider_id).await;
    if let Some(t) = old_token.as_deref() {
        if !t.is_empty() {
            crate::modules::llm_local_runtime::proxy::remove_token(t).await;
        }
    }

    sync_publish(
        SyncEntity::LlmProvider,
        SyncAction::Update,
        provider_id,
        Audience::perm::<LlmProvidersRead>(),
        origin.0,
    );
    sync_publish(
        SyncEntity::UserLlmProvider,
        SyncAction::Update,
        provider_id,
        Audience::perm::<UserLlmProvidersRead>(),
        origin.0,
    );

    Ok((
        StatusCode::OK,
        Json(RotateProxyTokenResponse {
            provider: updated,
            plaintext_api_key: new_token,
        }),
    ))
}

pub fn rotate_proxy_token_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersEdit,)>(op)
        .id("LlmProvider.rotateProxyToken")
        .tag("LLM Providers")
        .summary("Rotate the PROXY_TOKEN on a local provider.")
        .response::<200, Json<RotateProxyTokenResponse>>()
        .response_with::<400, (), _>(|r| r.description("Not a local provider"))
        .response_with::<404, (), _>(|r| r.description("Provider not found"))
}

/// Update an existing LLM provider (requires llm_providers::edit permission)
#[debug_handler]
pub async fn update_provider(
    _auth: RequirePermissions<(LlmProvidersEdit,)>,
    Path(provider_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<UpdateLlmProviderRequest>,
) -> ApiResult<Json<LlmProvider>> {
    // Validate request
    utils::validate_update_request(&request)?;

    // Local providers authenticate via a server-minted proxy token, not a
    // user/admin-typed api_key. Accepting an api_key here would overwrite the
    // minted token in the DB WITHOUT syncing the in-memory proxy token cache,
    // breaking local inference. Token changes go through the rotate-proxy-token
    // endpoint. Only pay for the extra lookup when an api_key is actually set.
    if request.api_key.is_some() {
        let existing = Repos
            .llm_provider
            .get_by_id(provider_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get provider {}: {}", provider_id, e);
                AppError::internal_error("Database operation failed")
            })?
            .ok_or_else(|| AppError::not_found("Provider"))?;
        if existing.provider_type == "local" {
            return Err(AppError::bad_request(
                "PROVIDER_IS_LOCAL",
                "Local providers use a server-minted proxy token; rotate it via the \
                 rotate-proxy-token endpoint instead of setting an api_key",
            )
            .into());
        }
    }

    // Update provider
    let provider = Repos.llm_provider
        .update(provider_id, request)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    // Emit event
    event_bus.emit_async(LlmProviderEvent::updated(provider.clone()).into());

    sync_publish(
        SyncEntity::LlmProvider,
        SyncAction::Update,
        provider.id,
        Audience::perm::<LlmProvidersRead>(),
        origin.0,
    );
    sync_publish(
        SyncEntity::UserLlmProvider,
        SyncAction::Update,
        provider.id,
        Audience::perm::<UserLlmProvidersRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(provider)))
}

pub fn update_provider_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersEdit,)>(op)
        .id("LlmProvider.update")
        .tag("LLM Providers")
        .summary("Update an existing LLM provider")
        .response::<200, Json<LlmProvider>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Provider not found"))
}

/// Delete an LLM provider (requires llm_providers::delete permission)
#[debug_handler]
pub async fn delete_provider(
    _auth: RequirePermissions<(LlmProvidersDelete,)>,
    Path(provider_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Get provider info before deleting (for event emission)
    let provider = Repos.llm_provider.get_by_id(provider_id).await.map_err(|e| {
        tracing::error!("Failed to get provider {}: {}", provider_id, e);
        AppError::internal_error("Database operation failed")
    })?;

    match Repos.llm_provider.delete(provider_id).await {
        Ok(Ok(true)) => {
            // Emit event with provider name
            if let Some(p) = provider {
                event_bus.emit_async(LlmProviderEvent::deleted(provider_id, p.name).into());
            }
            sync_publish(
                SyncEntity::LlmProvider,
                SyncAction::Delete,
                provider_id,
                Audience::perm::<LlmProvidersRead>(),
                origin.0,
            );
            sync_publish(
                SyncEntity::UserLlmProvider,
                SyncAction::Delete,
                provider_id,
                Audience::perm::<UserLlmProvidersRead>(),
                origin.0,
            );
            // Best-effort: remove the provider's on-disk model directory tree
            // (`<app_data>/models/<provider_id>/`). The DB cascade dropped the
            // model rows; without this their downloaded files are orphaned.
            if let Ok(storage) =
                crate::modules::llm_model::storage::ModelStorage::new().await
            {
                let dir = storage.get_provider_dir(&provider_id);
                if dir.exists() {
                    if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
                        tracing::warn!(
                            "delete_provider: failed to remove model dir {}: {}",
                            dir.display(),
                            e
                        );
                    }
                }
            }
            Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
        }
        // Already gone (e.g. a concurrent DELETE won the race). DELETE is
        // idempotent: report success rather than a spurious 404. No event /
        // sync emit — this request changed nothing.
        Ok(Ok(false)) => Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT)),
        Ok(Err(msg)) => Err(AppError::bad_request("DELETE_ERROR", &msg).into()),
        Err(e) => {
            tracing::error!("Failed to delete provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed").into())
        }
    }
}

pub fn delete_provider_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersDelete,)>(op)
        .id("LlmProvider.delete")
        .tag("LLM Providers")
        .summary("Delete an LLM provider")
        .description("Delete a non-built-in LLM provider. Built-in providers cannot be deleted.")
        .response_with::<204, (), _>(|res| res.description("Provider deleted successfully"))
        .response_with::<400, (), _>(|res| res.description("Cannot delete built-in provider"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Provider not found"))
        .response_with::<500, (), _>(|res| res.description("Internal server error"))
}
