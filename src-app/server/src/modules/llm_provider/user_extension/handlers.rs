// HTTP handlers for the provider↔group join surface.
// Relocated verbatim from `llm_provider/handlers/admin.rs:355-590` as
// part of the llm_provider↔user/Group inversion.
//
// Why this lives here: every handler either returns `Vec<Group>` (user
// type) or operates on the `user_group_llm_providers` join. Importing
// `user::models::Group` is the architecturally-correct direction inside
// the `user_extension/` bridge subdir.
//
// **OpenAPI `.id()` strings are preserved verbatim** — the autogen
// `ApiClient.LlmProvider.getGroups/assignGroup/removeGroup` +
// `ApiClient.Group.getProviders/updateProviders` names depend on them.
// Don't rename without confirming the 4 frontend consumers
// (ProviderGroupAssignmentCard, LlmProviderGroupsAssignmentDrawer,
// GroupLlmProvidersAssignmentDrawer, LLMProviderGroupWidget).

use axum::{
    Extension, Json, debug_handler,
    extract::Path,
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::{events::EventBus, repository::Repos};
use crate::modules::llm_provider::events::LlmProviderEvent;
use crate::modules::llm_provider::permissions::{LlmProvidersAssignGroups, LlmProvidersRead};
use crate::modules::llm_provider::types::{
    AssignProviderToGroupRequest, GroupProvidersResponse, UpdateGroupProvidersRequest,
};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};
use crate::modules::user::models::Group;

/// Get all groups assigned to a provider (requires llm_providers::read permission)
#[debug_handler]
pub async fn get_provider_groups(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Group>>> {
    let groups = Repos
        .user_group_llm_provider
        .get_provider_groups(provider_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get groups for provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(groups)))
}

pub fn get_provider_groups_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.getGroups")
        .tag("LLM Providers")
        .summary("Get groups assigned to a provider")
        .response::<200, Json<Vec<Group>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Assign a provider to a user group (requires llm_providers::assign_groups permission)
#[debug_handler]
pub async fn assign_provider_to_group(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path(provider_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<AssignProviderToGroupRequest>,
) -> ApiResult<StatusCode> {
    Repos
        .user_group_llm_provider
        .assign_to_group(provider_id, request.group_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to assign provider {} to group {}: {}",
                provider_id, request.group_id, e
            );
            AppError::internal_error("Database operation failed")
        })?;

    // Get updated group list for event
    let groups = Repos
        .user_group_llm_provider
        .get_provider_groups(provider_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get groups for provider {}: {}", provider_id, e);
            AppError::internal_error("Database operation failed")
        })?;
    let group_ids: Vec<Uuid> = groups.iter().map(|g| g.id).collect();

    // Emit event
    event_bus.emit_async(LlmProviderEvent::group_assignment_changed(provider_id, group_ids).into());

    // A provider's group visibility changed → notify both audiences: admins
    // (provider table) and every user (their accessible provider set may have
    // changed — each refetches its own group-scoped view).
    sync_publish(SyncEntity::LlmProvider, SyncAction::Update, provider_id, None, origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, provider_id, None, origin.0);

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn assign_provider_to_group_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("LlmProvider.assignGroup")
        .tag("LLM Providers")
        .summary("Assign a provider to a user group")
        .response_with::<204, (), _>(|res| {
            res.description("Provider assigned to group successfully")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Remove a provider from a user group (requires llm_providers::assign_groups permission)
#[debug_handler]
pub async fn remove_provider_from_group(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path((provider_id, group_id)): Path<(Uuid, Uuid)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    let removed = Repos
        .user_group_llm_provider
        .remove_from_group(group_id, provider_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to remove provider {} from group {}: {}",
                provider_id, group_id, e
            );
            AppError::internal_error("Database operation failed")
        })?;

    if removed {
        // Get updated group list for event
        let groups = Repos
            .user_group_llm_provider
            .get_provider_groups(provider_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get groups for provider {}: {}", provider_id, e);
                AppError::internal_error("Database operation failed")
            })?;
        let group_ids: Vec<Uuid> = groups.iter().map(|g| g.id).collect();

        // Emit event
        event_bus
            .emit_async(LlmProviderEvent::group_assignment_changed(provider_id, group_ids).into());

        sync_publish(SyncEntity::LlmProvider, SyncAction::Update, provider_id, None, origin.0);
        sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, provider_id, None, origin.0);

        Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
    } else {
        Err(AppError::not_found("Provider group assignment").into())
    }
}

pub fn remove_provider_from_group_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("LlmProvider.removeGroup")
        .tag("LLM Providers")
        .summary("Remove a provider from a user group")
        .response_with::<204, (), _>(|res| {
            res.description("Provider removed from group successfully")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Assignment not found"))
}

// =====================================================
// Group-Centric Handlers (for UI widgets)
// =====================================================

/// Get all providers assigned to a group (requires llm_providers::read permission)
#[debug_handler]
pub async fn get_group_providers(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(group_id): Path<Uuid>,
) -> ApiResult<Json<GroupProvidersResponse>> {
    let providers = Repos
        .user_group_llm_provider
        .get_for_group(group_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get providers for group {}: {}", group_id, e);
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupProvidersResponse { providers })))
}

pub fn get_group_providers_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("Group.getProviders")
        .tag("Admin - Groups")
        .summary("Get all providers assigned to a group")
        .response::<200, Json<GroupProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Bulk update providers for a group (requires llm_providers::assign_groups permission)
/// Atomically updates provider assignments - adds new providers and removes unspecified ones
#[debug_handler]
pub async fn update_group_providers(
    _auth: RequirePermissions<(LlmProvidersAssignGroups,)>,
    Path(group_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<UpdateGroupProvidersRequest>,
) -> ApiResult<Json<GroupProvidersResponse>> {
    use std::collections::HashSet;

    // Get current assignments
    let current = Repos
        .user_group_llm_provider
        .get_for_group(group_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to get current providers for group {}: {}",
                group_id, e
            );
            AppError::internal_error("Database operation failed")
        })?;

    let current_ids: HashSet<Uuid> = current.iter().map(|p| p.id).collect();
    let new_ids: HashSet<Uuid> = request.provider_ids.iter().copied().collect();

    // Calculate diff
    let to_add: Vec<Uuid> = new_ids.difference(&current_ids).copied().collect();
    let to_remove: Vec<Uuid> = current_ids.difference(&new_ids).copied().collect();

    // Track all affected providers for event emission
    let mut affected_provider_ids: HashSet<Uuid> = HashSet::new();

    // Apply changes - remove first, then add
    for provider_id in to_remove {
        Repos
            .user_group_llm_provider
            .remove_from_group(group_id, provider_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to remove provider {} from group {}: {}",
                    provider_id, group_id, e
                );
                AppError::internal_error("Database operation failed")
            })?;
        affected_provider_ids.insert(provider_id);
    }

    for provider_id in to_add {
        Repos
            .user_group_llm_provider
            .assign_to_group(provider_id, group_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to assign provider {} to group {}: {}",
                    provider_id, group_id, e
                );
                AppError::internal_error("Database operation failed")
            })?;
        affected_provider_ids.insert(provider_id);
    }

    // Emit events for all affected providers
    for provider_id in affected_provider_ids {
        let groups = Repos
            .user_group_llm_provider
            .get_provider_groups(provider_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get groups for provider {}: {}", provider_id, e);
                AppError::internal_error("Database operation failed")
            })?;
        let group_ids: Vec<Uuid> = groups.iter().map(|g| g.id).collect();
        event_bus
            .emit_async(LlmProviderEvent::group_assignment_changed(provider_id, group_ids).into());
        sync_publish(SyncEntity::LlmProvider, SyncAction::Update, provider_id, None, origin.0);
        sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, provider_id, None, origin.0);
    }

    // Return updated list
    let providers = Repos
        .user_group_llm_provider
        .get_for_group(group_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to get updated providers for group {}: {}",
                group_id, e
            );
            AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupProvidersResponse { providers })))
}

pub fn update_group_providers_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(LlmProvidersAssignGroups,)>(op)
        .id("Group.updateProviders")
        .tag("Admin - Groups")
        .summary("Update providers assigned to a group")
        .description("Atomically updates provider assignments. Adds new providers and removes unspecified ones.")
        .response::<200, Json<GroupProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
