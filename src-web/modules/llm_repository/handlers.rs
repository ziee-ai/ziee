// LLM Repository handlers - copied from react-test and refactored for ziee-chat
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: ALL validation logic preserved from react-test

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::permissions::RequirePermissions,
};

use super::{
    models::LlmRepository,
    types::{
        CreateLlmRepositoryRequest, LlmRepositoryListResponse,
        TestRepositoryConnectionRequest, TestRepositoryConnectionResponse,
        UpdateLlmRepositoryRequest,
    },
    permissions::*,
    repository::LlmRepositoryRepository,
    utils,
};

// =====================================================
// Route Handlers
// =====================================================

/// List all LLM repositories (requires llm_repositories::read permission)
pub async fn list_repositories(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Query(params): Query<PaginationQuery>,
    Extension(repo): Extension<LlmRepositoryRepository>,
) -> ApiResult<Json<LlmRepositoryListResponse>> {
    // Get all repositories
    let all_repositories = repo.list().await
        .map_err(|e| {
            eprintln!("Failed to get repositories: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    // Calculate pagination
    let total = all_repositories.len() as i64;
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(all_repositories.len());

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

/// Get LLM repository by ID (requires llm_repositories::read permission)
pub async fn get_repository(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Path(repository_id): Path<Uuid>,
    Extension(repo): Extension<LlmRepositoryRepository>,
) -> ApiResult<Json<LlmRepository>> {
    let repository = repo.get_by_id(repository_id).await
        .map_err(|e| {
            eprintln!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(repository)))
}

/// Create a new LLM repository (requires llm_repositories::create permission)
/// ALL validation logic preserved from react-test
pub async fn create_repository(
    _auth: RequirePermissions<(LlmRepositoriesCreate,)>,
    Extension(repo): Extension<LlmRepositoryRepository>,
    Json(request): Json<CreateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type
    utils::validate_auth_type(&request.auth_type)?;

    // Validate URL format
    utils::validate_url(&request.url)?;

    // Validate authentication configuration
    utils::validate_auth_config_for_create(&request)?;

    // Create repository
    let repository = repo.create(request).await
        .map_err(|e| {
            eprintln!("Failed to create repository: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::CREATED, Json(repository)))
}

/// Update an existing LLM repository (requires llm_repositories::edit permission)
/// ALL validation logic preserved from react-test including auth_config merging
pub async fn update_repository(
    _auth: RequirePermissions<(LlmRepositoriesEdit,)>,
    Path(repository_id): Path<Uuid>,
    Extension(repo): Extension<LlmRepositoryRepository>,
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
    let current_repository = repo.get_by_id(repository_id).await
        .map_err(|e| {
            eprintln!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // Validate auth fields based on auth type (use current or new values)
    utils::validate_auth_config_for_update(&current_repository, &request)?;

    // Update repository
    let updated_repository = repo.update(repository_id, request).await
        .map_err(|e| {
            eprintln!("Failed to update repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(updated_repository)))
}

/// Delete an LLM repository (requires llm_repositories::delete permission)
/// Built-in repositories cannot be deleted
pub async fn delete_repository(
    _auth: RequirePermissions<(LlmRepositoriesDelete,)>,
    Path(repository_id): Path<Uuid>,
    Extension(repo): Extension<LlmRepositoryRepository>,
) -> ApiResult<StatusCode> {
    match repo.delete(repository_id).await {
        Ok(Ok(true)) => Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT)),
        Ok(Ok(false)) => Err(AppError::not_found("Repository").into()),
        Ok(Err(error_message)) => {
            eprintln!("Cannot delete repository {}: {}", repository_id, error_message);
            Err(AppError::bad_request(
                "INVALID_OPERATION",
                "Cannot delete built-in repository",
            ).into())
        }
        Err(e) => {
            eprintln!("Failed to delete repository {}: {}", repository_id, e);
            Err(AppError::internal_error("Database operation failed").into())
        }
    }
}

/// Test LLM repository connection (requires llm_repositories::read permission)
/// Tests connectivity with provided credentials without saving
/// ALL logic preserved from react-test including Hugging Face special handling
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
