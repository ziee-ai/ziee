// Group handlers

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

use crate::modules::user::{
    models::Group,
    permissions::*,
    types::{
        AssignUserToGroupRequest, CreateGroupRequest, GroupListResponse, UpdateGroupRequest,
        UserListResponse,
    },
};

// =====================================================
// Route Handlers
// =====================================================

/// List all groups (requires groups::read permission)
#[debug_handler]
pub async fn list_groups(
    _auth: RequirePermissions<(GroupsRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<GroupListResponse>> {
    let (groups, total) = Repos.group.list(params.page, params.per_page).await?;

    let total_pages = (total + params.per_page as i64 - 1) / params.per_page as i64;

    Ok((
        StatusCode::OK,
        Json(GroupListResponse {
            groups,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        }),
    ))
}

/// Documentation for list_groups endpoint
pub fn list_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsRead,)>(op)
        .id("UserGroup.list")
        .tag("User Groups")
        .summary("List all groups with pagination")
        .response::<200, Json<GroupListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get group by ID (requires groups::read permission)
#[debug_handler]
pub async fn get_group(
    _auth: RequirePermissions<(GroupsRead,)>,
    Path(group_id): Path<Uuid>,
) -> ApiResult<Json<Group>> {
    let group = Repos
        .group
        .get_by_id(group_id)
        .await?
        .ok_or_else(|| AppError::not_found("Group"))?;

    Ok((StatusCode::OK, Json(group)))
}

/// Documentation for get_group endpoint
pub fn get_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsRead,)>(op)
        .id("UserGroup.get")
        .tag("User Groups")
        .summary("Get group by ID")
        .response::<200, Json<Group>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Group not found"))
}

/// Create a new group (requires groups::create permission)
#[debug_handler]
pub async fn create_group(
    _auth: RequirePermissions<(GroupsCreate,)>,
    origin: SyncOrigin,
    Json(request): Json<CreateGroupRequest>,
) -> ApiResult<Json<Group>> {
    // Validate group name
    if request.name.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Group name cannot be empty").into());
    }

    // Check if group name already exists
    if Repos.group.get_by_name(&request.name).await?.is_some() {
        return Err(AppError::conflict("Group name").into());
    }

    // Create group
    let group = Repos
        .group
        .create(&request.name, request.description, request.permissions)
        .await?;

    sync_publish(
        SyncEntity::Group,
        SyncAction::Create,
        group.id,
        None,
        origin.0,
    );

    Ok((StatusCode::CREATED, Json(group)))
}

/// Documentation for create_group endpoint
pub fn create_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsCreate,)>(op)
        .id("UserGroup.create")
        .tag("User Groups")
        .summary("Create a new group")
        .response::<201, Json<Group>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<409, (), _>(|res| res.description("Group name already exists"))
}

/// Update group (requires groups::edit permission)
#[debug_handler]
pub async fn update_group(
    auth: RequirePermissions<(GroupsEdit,)>,
    Path(group_id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateGroupRequest>,
) -> ApiResult<Json<Group>> {
    // Check if group exists
    let existing_group = Repos
        .group
        .get_by_id(group_id)
        .await?
        .ok_or_else(|| AppError::not_found("Group"))?;

    // Prevent modification of system groups' core attributes — including
    // `permissions`. The original guard only covered name and is_active,
    // letting any groups::edit holder rewrite the default Users group's
    // permissions to ['*'] and cascade wildcard to every user (group
    // permissions union via check_permission_union). 02-permissions F-02
    // (High).
    if existing_group.is_system
        && (request.name.is_some()
            || request.is_active == Some(false)
            || request.permissions.is_some())
        {
            return Err(AppError::bad_request(
                "SYSTEM_GROUP",
                "Cannot modify name, deactivate, or change permissions of system groups",
            )
            .into());
        }

    // Prevent self-escalation: caller must hold every permission they're
    // trying to grant via this group (admins bypass). Same pattern as
    // create_user (03-user F-04). Closes the second half of 02-permissions
    // F-02.
    if let Some(ref requested_perms) = request.permissions
        && !auth.user.is_admin {
            for perm in requested_perms {
                if !crate::modules::permissions::checker::check_permission_union(
                    &auth.user,
                    &auth.groups,
                    perm,
                ) {
                    return Err(AppError::forbidden(
                        "CANNOT_GRANT_PERMISSION",
                        format!(
                            "Cannot grant permission '{}' that you do not hold yourself",
                            perm
                        ),
                    )
                    .into());
                }
            }
        }

    // Check if new name already exists
    if let Some(ref name) = request.name
        && let Some(existing) = Repos.group.get_by_name(name).await?
            && existing.id != group_id {
                return Err(AppError::conflict("Group name").into());
            }

    // Update group
    let group = Repos
        .group
        .update(
            group_id,
            request.name,
            request.description,
            request.permissions,
            request.is_active,
        )
        .await?;

    sync_publish(
        SyncEntity::Group,
        SyncAction::Update,
        group.id,
        None,
        origin.0,
    );
    // Editing a group's permissions changes the effective permissions of
    // EVERY member, so fan a permissions-changed signal out to each (Owner-
    // scoped) — their devices re-bootstrap /auth/me immediately rather than
    // waiting up to 60s for the per-connection re-check. Batched into a single
    // registry-lock acquisition (the default Users group can contain every
    // user). Best-effort: a query failure just falls back to the re-check.
    if let Ok(member_ids) = Repos.group.get_member_ids(group.id).await {
        crate::modules::sync::publish_session_to_users(&member_ids, origin.0);
    }

    Ok((StatusCode::OK, Json(group)))
}

/// Documentation for update_group endpoint
pub fn update_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsEdit,)>(op)
        .id("UserGroup.update")
        .tag("User Groups")
        .summary("Update group")
        .response::<200, Json<Group>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Group not found"))
        .response_with::<409, (), _>(|res| res.description("Group name already exists"))
}

/// Delete group (requires groups::delete permission)
#[debug_handler]
pub async fn delete_group(
    _auth: RequirePermissions<(GroupsDelete,)>,
    Path(group_id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Check if group exists
    let group = Repos
        .group
        .get_by_id(group_id)
        .await?
        .ok_or_else(|| AppError::not_found("Group"))?;

    // Prevent deletion of system groups
    if group.is_system {
        return Err(AppError::bad_request("SYSTEM_GROUP", "Cannot delete system groups").into());
    }

    // Delete group
    Repos.group.delete(group_id).await?;

    sync_publish(
        SyncEntity::Group,
        SyncAction::Delete,
        group_id,
        None,
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Documentation for delete_group endpoint
pub fn delete_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsDelete,)>(op)
        .id("UserGroup.delete")
        .tag("User Groups")
        .summary("Delete group")
        .response_with::<204, (), _>(|res| res.description("Group deleted successfully"))
        .response_with::<400, (), _>(|res| res.description("Cannot delete system group"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Group not found"))
}

/// Get members of a group (requires groups::read permission)
#[debug_handler]
pub async fn get_group_members(
    _auth: RequirePermissions<(GroupsRead,)>,
    Path(group_id): Path<Uuid>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<UserListResponse>> {
    // Check if group exists
    if Repos.group.get_by_id(group_id).await?.is_none() {
        return Err(AppError::not_found("Group").into());
    }

    // Get group members
    let (users, total) = Repos
        .group
        .get_members(group_id, params.page, params.per_page)
        .await?;

    let total_pages = (total + params.per_page as i64 - 1) / params.per_page as i64;

    Ok((
        StatusCode::OK,
        Json(UserListResponse {
            users,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        }),
    ))
}

/// Documentation for get_group_members endpoint
pub fn get_group_members_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsRead,)>(op)
        .id("UserGroup.getMembers")
        .tag("User Groups")
        .summary("Get members of a group")
        .response::<200, Json<UserListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Group not found"))
}

/// Assign user to group (requires groups::assign_users permission)
#[debug_handler]
pub async fn assign_user_to_group(
    auth: RequirePermissions<(GroupsAssignUsers,)>,
    origin: SyncOrigin,
    Json(request): Json<AssignUserToGroupRequest>,
) -> ApiResult<StatusCode> {
    // Check if user exists
    if Repos.user.get_by_id(request.user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Check if group exists
    if Repos.group.get_by_id(request.group_id).await?.is_none() {
        return Err(AppError::not_found("Group").into());
    }

    // Assign user to group
    Repos
        .user
        .assign_to_group(request.user_id, request.group_id, Some(auth.user.id))
        .await?;

    // Signal the affected user that their permissions changed so their
    // open sessions re-bootstrap /auth/me immediately (the 60s re-check is
    // the backstop). Owner-scoped to that user only.
    sync_publish(
        SyncEntity::Session,
        SyncAction::Update,
        request.user_id,
        Some(request.user_id),
        origin.0,
    );

    // The group's member list changed → refresh admins viewing it elsewhere.
    sync_publish(
        SyncEntity::Group,
        SyncAction::Update,
        request.group_id,
        None,
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Documentation for assign_user_to_group endpoint
pub fn assign_user_to_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsAssignUsers,)>(op)
        .id("UserGroup.assignUser")
        .tag("User Groups")
        .summary("Assign user to group")
        .response_with::<204, (), _>(|res| res.description("User assigned successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User or Group not found"))
}

/// Remove user from group (requires groups::assign_users permission)
#[debug_handler]
pub async fn remove_user_from_group(
    _auth: RequirePermissions<(GroupsAssignUsers,)>,
    Path((user_id, group_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Check if user exists
    if Repos.user.get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Check if group exists
    if Repos.group.get_by_id(group_id).await?.is_none() {
        return Err(AppError::not_found("Group").into());
    }

    // Remove user from group
    Repos.user.remove_from_group(user_id, group_id).await?;

    // Signal the affected user that their permissions changed (Owner-scoped).
    sync_publish(
        SyncEntity::Session,
        SyncAction::Update,
        user_id,
        Some(user_id),
        origin.0,
    );

    // The group's member list changed → refresh admins viewing it elsewhere.
    sync_publish(
        SyncEntity::Group,
        SyncAction::Update,
        group_id,
        None,
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Documentation for remove_user_from_group endpoint
pub fn remove_user_from_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(GroupsAssignUsers,)>(op)
        .id("UserGroup.removeUser")
        .tag("User Groups")
        .summary("Remove user from group")
        .response_with::<204, (), _>(|res| res.description("User removed successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User or Group not found"))
}
