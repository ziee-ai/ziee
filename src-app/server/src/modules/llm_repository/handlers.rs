// LLM Repository handlers - copied from react-test and refactored for ziee
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: ALL validation logic preserved from react-test

use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    core::{events::EventBus, repository::Repos},
    modules::permissions::{RequirePermissions, with_permission},
};
use std::sync::Arc;

use super::{
    events::LlmRepositoryEvent,
    models::LlmRepository,
    permissions::*,
    types::{
        CreateLlmRepositoryRequest, LlmRepositoryListResponse, TestRepositoryConnectionRequest,
        TestRepositoryConnectionResponse, UpdateLlmRepositoryRequest,
    },
    utils,
};

// =====================================================
// Route Handlers
// =====================================================

/// List all LLM repositories (requires llm_repositories::read permission)
#[debug_handler]
pub async fn list_repositories(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<LlmRepositoryListResponse>> {
    // Get all repositories
    let all_repositories = Repos.llm_repository.list().await.map_err(|e| {
        tracing::error!("Failed to get repositories: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Calculate pagination. Cast to i64 before multiply so the
    // PaginationQuery clamp can't be circumvented at the multiply
    // (defense-in-depth; the deserializer already bounds the inputs).
    // Closes 09-llm-repository F-11 (Medium).
    let total = all_repositories.len() as i64;
    let start = ((params.page as i64).saturating_sub(1))
        .saturating_mul(params.per_page as i64)
        .max(0) as usize;
    let end = start
        .saturating_add(params.per_page as usize)
        .min(all_repositories.len());

    let paginated_repositories = if start < all_repositories.len() {
        all_repositories[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok((
        StatusCode::OK,
        Json(LlmRepositoryListResponse {
            repositories: paginated_repositories,
            total,
            page: params.page,
            per_page: params.per_page,
        }),
    ))
}

/// Documentation for list_repositories endpoint
pub fn list_repositories_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.list")
        .tag("LLM Repositories")
        .summary("List all LLM repositories with pagination")
        .response::<200, Json<LlmRepositoryListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM repository by ID (requires llm_repositories::read permission)
#[debug_handler]
pub async fn get_repository(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<Json<LlmRepository>> {
    let repository = Repos.llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(repository)))
}

/// Documentation for get_repository endpoint
pub fn get_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.get")
        .tag("LLM Repositories")
        .summary("Get LLM repository by ID")
        .response::<200, Json<LlmRepository>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Create a new LLM repository (requires llm_repositories::create permission)
/// ALL validation logic preserved from react-test
#[debug_handler]
pub async fn create_repository(
    _auth: RequirePermissions<(LlmRepositoriesCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type
    utils::validate_auth_type(&request.auth_type)?;

    // Validate URL format
    utils::validate_url(&request.url)?;

    // Validate authentication configuration
    utils::validate_auth_config_for_create(&request)?;

    // Create repository
    let repository = Repos.llm_repository.create(request).await.map_err(|e| {
        tracing::error!("Failed to create repository: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Emit event
    event_bus.emit_async(LlmRepositoryEvent::created(repository.clone()).into());

    Ok((StatusCode::CREATED, Json(repository)))
}

/// Documentation for create_repository endpoint
pub fn create_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesCreate,)>(op)
        .id("LlmRepository.create")
        .tag("LLM Repositories")
        .summary("Create a new LLM repository")
        .response::<201, Json<LlmRepository>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM repository (requires llm_repositories::edit permission)
/// ALL validation logic preserved from react-test including auth_config merging
#[debug_handler]
pub async fn update_repository(
    _auth: RequirePermissions<(LlmRepositoriesEdit,)>,
    Path(repository_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<UpdateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type if provided
    if let Some(ref auth_type) = request.auth_type {
        utils::validate_auth_type(auth_type)?;
    }

    // Validate URL format if provided
    if let Some(ref url) = request.url {
        utils::validate_url(url)?;
    }

    // Get current repository to validate auth config updates
    let current_repository = Repos.llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // SECURITY: refuse to mutate the built-in repository's URL,
    // auth_type, or name. The delete handler already blocked this
    // for built-in repos, but the update handler didn't — so any
    // holder of llm_repositories::edit could swap the Hugging Face
    // URL to an attacker-controlled domain, then watch tokens flow
    // there on the next model download. Closes 09-llm-repository F-16.
    //
    // `auth_config` IS allowed to mutate (originally blocked, then
    // relaxed): the built-in HF repository ships with an empty
    // `api_key` in its seed `auth_config` and the operator must
    // populate it before downloads will authenticate. Blocking
    // auth_config writes would mean operators can never set the HF
    // token without an out-of-band migration. The URL/auth_type/name
    // immutability is sufficient — a malicious operator can supply a
    // bad api_key, but that exfiltrates only to the legitimate (still
    // pinned) HF URL.
    if current_repository.built_in {
        let touches_immutable = request.url.is_some()
            || request.auth_type.is_some()
            || request.name.is_some();
        if touches_immutable {
            return Err(AppError::bad_request(
                "BUILT_IN_REPOSITORY",
                "Cannot modify name / URL / auth_type on built-in repositories",
            )
            .into());
        }
    }

    // Validate auth fields based on auth type (use current or new values)
    utils::validate_auth_config_for_update(&current_repository, &request)?;

    // Update repository
    let updated_repository = Repos.llm_repository
        .update(repository_id, request)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // Emit event
    event_bus.emit_async(LlmRepositoryEvent::updated(updated_repository.clone()).into());

    Ok((StatusCode::OK, Json(updated_repository)))
}

/// Documentation for update_repository endpoint
pub fn update_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesEdit,)>(op)
        .id("LlmRepository.update")
        .tag("LLM Repositories")
        .summary("Update an existing LLM repository")
        .response::<200, Json<LlmRepository>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Delete an LLM repository (requires llm_repositories::delete permission)
/// Built-in repositories cannot be deleted
#[debug_handler]
pub async fn delete_repository(
    _auth: RequirePermissions<(LlmRepositoriesDelete,)>,
    Path(repository_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<StatusCode> {
    // Get repository name before deletion for event
    let repository_name = Repos.llm_repository
        .get_by_id(repository_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    match Repos.llm_repository.delete(repository_id).await {
        Ok(Ok(true)) => {
            // Emit event
            event_bus
                .emit_async(LlmRepositoryEvent::deleted(repository_id, repository_name).into());
            Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
        }
        Ok(Ok(false)) => Err(AppError::not_found("Repository").into()),
        Ok(Err(error_message)) => {
            tracing::error!(
                "Cannot delete repository {}: {}",
                repository_id, error_message
            );
            Err(
                AppError::bad_request("INVALID_OPERATION", "Cannot delete built-in repository")
                    .into(),
            )
        }
        Err(e) => {
            tracing::error!("Failed to delete repository {}: {}", repository_id, e);
            Err(AppError::internal_error("Database operation failed").into())
        }
    }
}

/// Documentation for delete_repository endpoint
pub fn delete_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesDelete,)>(op)
        .id("LlmRepository.delete")
        .tag("LLM Repositories")
        .summary("Delete an LLM repository")
        .response::<204, ()>()
        .response_with::<400, (), _>(|res| res.description("Cannot delete built-in repository"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Test LLM repository connection (requires llm_repositories::read permission)
/// Tests connectivity with provided credentials without saving
/// ALL logic preserved from react-test including Hugging Face special handling
#[debug_handler]
pub async fn test_repository_connection(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Json(request): Json<TestRepositoryConnectionRequest>,
) -> ApiResult<Json<TestRepositoryConnectionResponse>> {
    // Validate URL format
    if utils::validate_url(&request.url).is_err() {
        return Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: "Invalid URL format".to_string(),
            }),
        ));
    }

    // Test the repository connection
    match utils::test_repository_connectivity(&request).await {
        Ok(()) => Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: true,
                message: format!("Connection to {} successful", request.name),
            }),
        )),
        Err(e) => Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: format!("Connection to {} failed: {}", request.name, e),
            }),
        )),
    }
}

/// Documentation for test_repository_connection endpoint
pub fn test_repository_connection_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.test")
        .tag("LLM Repositories")
        .summary("Test repository connection without saving")
        .response::<200, Json<TestRepositoryConnectionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
