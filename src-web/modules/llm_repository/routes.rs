// LLM Repository routes and handlers - copied from react-test and refactored for ziee-chat
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: ALL validation logic preserved from react-test

use aide::axum::{routing::{get_with, post_with}, ApiRouter};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::{
    models::{
        CreateLlmRepositoryRequest, LlmRepository, LlmRepositoryListResponse,
        TestRepositoryConnectionRequest, TestRepositoryConnectionResponse,
        UpdateLlmRepositoryRequest,
    },
    permissions::*,
    repository,
    service,
};

/// LLM Repository management routes
pub fn llm_repository_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route("/llm-repositories", get_with(list_repositories, list_repositories_docs))
        .api_route("/llm-repositories", post_with(create_repository, create_repository_docs))
        .api_route("/llm-repositories/test", post_with(test_repository_connection, test_repository_connection_docs))
        .api_route("/llm-repositories/{repository_id}", get_with(get_repository, get_repository_docs))
        .api_route("/llm-repositories/{repository_id}", post_with(update_repository, update_repository_docs))
        .api_route("/llm-repositories/{repository_id}", aide::axum::routing::delete_with(delete_repository, delete_repository_docs))
}

// =====================================================
// Route Handlers
// =====================================================

/// List all LLM repositories (requires llm_repositories::read permission)
async fn list_repositories(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Query(params): Query<PaginationQuery>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmRepositoryListResponse>> {
    // Get all repositories
    let all_repositories = repository::list_llm_repositories(&pool).await
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

fn list_repositories_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.list")
        .tag("LLM Repositories")
        .summary("List all LLM repositories with pagination")
        .response::<200, Json<LlmRepositoryListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM repository by ID (requires llm_repositories::read permission)
async fn get_repository(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Path(repository_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<LlmRepository>> {
    let repository = repository::get_llm_repository_by_id(&pool, repository_id).await
        .map_err(|e| {
            eprintln!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(repository)))
}

fn get_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
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
async fn create_repository(
    _auth: RequirePermissions<(LlmRepositoriesCreate,)>,
    State(pool): State<PgPool>,
    Json(request): Json<CreateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type
    service::validate_auth_type(&request.auth_type)?;

    // Validate URL format
    service::validate_url(&request.url)?;

    // Validate authentication configuration
    service::validate_auth_config_for_create(&request)?;

    // Create repository
    let repository = repository::create_llm_repository(&pool, request).await
        .map_err(|e| {
            eprintln!("Failed to create repository: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::CREATED, Json(repository)))
}

fn create_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
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
async fn update_repository(
    _auth: RequirePermissions<(LlmRepositoriesEdit,)>,
    Path(repository_id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(request): Json<UpdateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type if provided
    if let Some(ref auth_type) = request.auth_type {
        service::validate_auth_type(auth_type)?;
    }

    // Validate URL format if provided
    if let Some(ref url) = request.url {
        service::validate_url(url)?;
    }

    // Get current repository to validate auth config updates
    let current_repository = repository::get_llm_repository_by_id(&pool, repository_id).await
        .map_err(|e| {
            eprintln!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // Validate auth fields based on auth type (use current or new values)
    service::validate_auth_config_for_update(&current_repository, &request)?;

    // Update repository
    let updated_repository = repository::update_llm_repository(&pool, repository_id, request).await
        .map_err(|e| {
            eprintln!("Failed to update repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(updated_repository)))
}

fn update_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
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
async fn delete_repository(
    _auth: RequirePermissions<(LlmRepositoriesDelete,)>,
    Path(repository_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<StatusCode> {
    match repository::delete_llm_repository(&pool, repository_id).await {
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

fn delete_repository_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
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
async fn test_repository_connection(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Json(request): Json<TestRepositoryConnectionRequest>,
) -> ApiResult<Json<TestRepositoryConnectionResponse>> {
    // Validate URL format
    if service::validate_url(&request.url).is_err() {
        return Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: "Invalid URL format".to_string(),
            }),
        ));
    }

    // Test the repository connection
    match service::test_repository_connectivity(&request).await {
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

fn test_repository_connection_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.test")
        .tag("LLM Repositories")
        .summary("Test repository connection without saving")
        .response::<200, Json<TestRepositoryConnectionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
