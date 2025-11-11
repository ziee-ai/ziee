// LLM Provider handlers

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::{permissions::{RequirePermissions, with_permission}, user::models::Group},
};

use super::{
    models::LlmProvider,
    permissions::*,
    repository::LlmProviderRepository,
    utils,
    types::{AssignProviderToGroupRequest, CreateLlmProviderRequest, GroupProvidersResponse, LlmProviderListResponse, UpdateGroupProvidersRequest, UpdateLlmProviderRequest},
};

// =====================================================
// Provider CRUD Handlers
// =====================================================

/// List all LLM providers (requires llm_providers::read permission)
pub async fn list_providers(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Query(params): Query<PaginationQuery>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<Json<LlmProviderListResponse>> {
    // Get all providers
    let all_providers = repo.list().await
        .map_err(|e| {
            eprintln!("Failed to get providers: {}", e);
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

pub fn list_providers_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.list")
        .tag("LLM Providers")
        .summary("List all LLM providers with pagination")
        .response::<200, Json<LlmProviderListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM provider by ID (requires llm_providers::read permission)
pub async fn get_provider(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(provider_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<Json<LlmProvider>> {
    let provider = repo.get_by_id(provider_id).await
        .map_err(|e| {
            eprintln!("Failed to get provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    Ok((StatusCode::OK, Json(provider)))
}

pub fn get_provider_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.get")
        .tag("LLM Providers")
        .summary("Get LLM provider by ID")
        .response::<200, Json<LlmProvider>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Provider not found"))
}

/// Create a new LLM provider (requires llm_providers::create permission)
pub async fn create_provider(
    _auth: RequirePermissions<(LlmProvidersCreate,)>,
    Extension(repo): Extension<LlmProviderRepository>,
    Json(request): Json<CreateLlmProviderRequest>,
) -> ApiResult<Json<LlmProvider>> {
    // Validate request
    utils::validate_create_request(&request)?;

    // Create provider
    let provider = repo.create(request).await
        .map_err(|e| {
            eprintln!("Failed to create provider: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::CREATED, Json(provider)))
}

pub fn create_provider_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersCreate,)>(op)
        .id("LlmProvider.create")
        .tag("LLM Providers")
        .summary("Create a new LLM provider")
        .response::<201, Json<LlmProvider>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM provider (requires llm_providers::edit permission)
pub async fn update_provider(
    _auth: RequirePermissions<(LlmProvidersEdit,)>,
    Path(provider_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
    Json(request): Json<UpdateLlmProviderRequest>,
) -> ApiResult<Json<LlmProvider>> {
    // Validate request
    utils::validate_update_request(&request)?;

    // Update provider
    let provider = repo.update(provider_id, request).await
        .map_err(|e| {
            eprintln!("Failed to update provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    Ok((StatusCode::OK, Json(provider)))
}

pub fn update_provider_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
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
pub async fn delete_provider(
    _auth: RequirePermissions<(LlmProvidersDelete,)>,
    Path(provider_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<StatusCode> {
    match repo.delete(provider_id).await {
        Ok(Ok(true)) => Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT)),
        Ok(Ok(false)) => Err(AppError::not_found("Provider").into()),
        Ok(Err(msg)) => Err(AppError::bad_request("DELETE_ERROR", &msg).into()),
        Err(e) => {
            eprintln!("Failed to delete provider {}: {}", provider_id, e);
            Err(AppError::internal_error("Database operation failed").into())
        }
    }
}

pub fn delete_provider_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersDelete,)>(op)
        .id("LlmProvider.delete")
        .tag("LLM Providers")
        .summary("Delete an LLM provider")
        .description("Delete a non-built-in LLM provider. Built-in providers cannot be deleted.")
        .response_with::<204, (), _>(|res| res.description("Provider deleted successfully"))
        .response_with::<400, (), _>(|res| res.description("Cannot delete built-in provider"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Provider not found"))
}

// =====================================================
// Group Assignment Handlers
// =====================================================

/// Get all groups assigned to a provider (requires llm_providers::read permission)
pub async fn get_provider_groups(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(provider_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<Json<Vec<Group>>> {
    let groups = repo.get_provider_groups(provider_id).await
        .map_err(|e| {
            eprintln!("Failed to get groups for provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(groups)))
}

pub fn get_provider_groups_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.getGroups")
        .tag("LLM Providers")
        .summary("Get groups assigned to a provider")
        .response::<200, Json<Vec<Group>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Assign a provider to a user group (requires llm_providers::assign_groups permission)
pub async fn assign_provider_to_group(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path(provider_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
    Json(request): Json<AssignProviderToGroupRequest>,
) -> ApiResult<StatusCode> {
    repo.assign_to_group(provider_id, request.group_id).await
        .map_err(|e| {
            eprintln!("Failed to assign provider {} to group {}: {}", provider_id, request.group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn assign_provider_to_group_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("LlmProvider.assignGroup")
        .tag("LLM Providers")
        .summary("Assign a provider to a user group")
        .response_with::<204, (), _>(|res| res.description("Provider assigned to group successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Remove a provider from a user group (requires llm_providers::assign_groups permission)
pub async fn remove_provider_from_group(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path((provider_id, group_id)): Path<(Uuid, Uuid)>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<StatusCode> {
    let removed = repo.remove_from_group(group_id, provider_id).await
        .map_err(|e| {
            eprintln!("Failed to remove provider {} from group {}: {}", provider_id, group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    if removed {
        Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
    } else {
        Err(AppError::not_found("Provider group assignment").into())
    }
}

pub fn remove_provider_from_group_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("LlmProvider.removeGroup")
        .tag("LLM Providers")
        .summary("Remove a provider from a user group")
        .response_with::<204, (), _>(|res| res.description("Provider removed from group successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Assignment not found"))
}

// =====================================================
// Group-Centric Handlers (for UI widgets)
// =====================================================

/// Get all providers assigned to a group (requires llm_providers::read permission)
/// This is a group-centric endpoint for the UI widget
pub async fn get_group_providers(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(group_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
) -> ApiResult<Json<GroupProvidersResponse>> {
    let providers = repo.get_for_group(group_id).await
        .map_err(|e| {
            eprintln!("Failed to get providers for group {}: {}", group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupProvidersResponse { providers })))
}

pub fn get_group_providers_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("Group.getProviders")
        .tag("Admin - Groups")
        .summary("Get all providers assigned to a group")
        .response::<200, Json<GroupProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Bulk update providers for a group (requires llm_providers::assign_groups permission)
/// Atomically updates provider assignments - adds new providers and removes unspecified ones
pub async fn update_group_providers(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path(group_id): Path<Uuid>,
    Extension(repo): Extension<LlmProviderRepository>,
    Json(request): Json<UpdateGroupProvidersRequest>,
) -> ApiResult<Json<GroupProvidersResponse>> {
    use std::collections::HashSet;

    // Get current assignments
    let current = repo.get_for_group(group_id).await
        .map_err(|e| {
            eprintln!("Failed to get current providers for group {}: {}", group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    let current_ids: HashSet<Uuid> = current.iter().map(|p| p.id).collect();
    let new_ids: HashSet<Uuid> = request.provider_ids.iter().copied().collect();

    // Calculate diff
    let to_add: Vec<Uuid> = new_ids.difference(&current_ids).copied().collect();
    let to_remove: Vec<Uuid> = current_ids.difference(&new_ids).copied().collect();

    // Apply changes - remove first, then add
    for provider_id in to_remove {
        repo.remove_from_group(group_id, provider_id).await
            .map_err(|e| {
                eprintln!("Failed to remove provider {} from group {}: {}", provider_id, group_id, e);
                AppError::internal_error("Database operation failed")
            })?;
    }

    for provider_id in to_add {
        repo.assign_to_group(provider_id, group_id).await
            .map_err(|e| {
                eprintln!("Failed to assign provider {} to group {}: {}", provider_id, group_id, e);
                AppError::internal_error("Database operation failed")
            })?;
    }

    // Return updated list
    let providers = repo.get_for_group(group_id).await
        .map_err(|e| {
            eprintln!("Failed to get updated providers for group {}: {}", group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupProvidersResponse { providers })))
}

pub fn update_group_providers_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("Group.updateProviders")
        .tag("Admin - Groups")
        .summary("Update providers assigned to a group")
        .description("Atomically updates provider assignments. Adds new providers and removes unspecified ones.")
        .response::<200, Json<GroupProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
