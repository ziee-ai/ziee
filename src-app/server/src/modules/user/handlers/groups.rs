// Group handlers

use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::auth::context::{AuthContext, AuthSyncAction, AuthSyncEntity},
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncOrigin},
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
// Validation
// =====================================================

/// Max group name length, in CHARACTERS.
///
/// `groups.name` is `character varying(100)` (see
/// `ziee-auth/migrations/202607140050_auth_schema.sql`) and Postgres bounds
/// varchar by characters, not bytes.
const GROUP_MAX_NAME_CHARS: usize = 100;

/// Reject blank, over-long and control-bearing group names.
///
/// Create checked only `is_empty()` and update checked nothing, so all three
/// of these reached the write and came back as a generic 500:
/// a 101-character name overflowed the column (`22001 value too long`), a name
/// carrying U+0000 could not be stored at all (`22021 invalid byte sequence`),
/// and `"   "` was persisted as an unnamed group. Control/bidi characters are
/// rejected for the same display-spoof reason `validate_assistant_name` does —
/// a group name is rendered in the admin list and in permission dialogs.
///
/// Extracted so it is Tier-1 unit-testable independently of the HTTP layer.
pub(crate) fn validate_group_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Group name cannot be empty",
        ));
    }
    if trimmed.chars().count() > GROUP_MAX_NAME_CHARS {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("Group name must be ≤ {GROUP_MAX_NAME_CHARS} characters"),
        ));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || crate::modules::auth::username::is_bidi_or_zero_width(c))
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Group name cannot contain control characters",
        ));
    }
    Ok(())
}

/// Reject a value Postgres cannot store (U+0000).
///
/// Thin wrapper over the shared `common::text_guard::reject_nul`, kept so the
/// existing call sites and their tests read unchanged. This used to be one of
/// three independent private copies of the same guard; that duplication is the
/// reason the read path (free-text query parameters) never got it.
pub(crate) fn reject_nul(value: &str, field: &str) -> Result<(), AppError> {
    crate::common::text_guard::reject_nul(value, field)
}

// =====================================================
// Route Handlers
// =====================================================

/// List all groups (requires groups::read permission)
#[debug_handler]
pub async fn list_groups(
    _auth: RequirePermissions<(GroupsRead,)>,
    Extension(ctx): Extension<AuthContext>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<GroupListResponse>> {
    let (groups, total) = ctx.group().list(params.page, params.per_page).await?;

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
    Extension(ctx): Extension<AuthContext>,
    Path(group_id): Path<Uuid>,
) -> ApiResult<Json<Group>> {
    let group = ctx.group()
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
    auth: RequirePermissions<(GroupsCreate,)>,
    Extension(ctx): Extension<AuthContext>,
    origin: SyncOrigin,
    Json(request): Json<CreateGroupRequest>,
) -> ApiResult<Json<Group>> {
    // Validate group name + description
    validate_group_name(&request.name)?;
    if let Some(ref description) = request.description {
        reject_nul(description, "Group description")?;
    }

    // Prevent self-escalation: caller must hold every permission they're
    // trying to grant via this group (admins bypass). Mirrors update_group —
    // create was missed, so a non-admin holding the delegable groups::create
    // could mint a group with permissions=['*'] (or 'users::*', etc.) and grant
    // itself more than it holds (group permissions union via
    // check_permission_union). 02-permissions F-02 / 03-user F-04 class.
    if !auth.user.is_admin {
        for perm in &request.permissions {
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

    // Check if group name already exists
    if ctx.group().get_by_name(&request.name).await?.is_some() {
        return Err(AppError::conflict("Group name").into());
    }

    // Create group
    let group = ctx.group()
        .create(&request.name, request.description, request.permissions)
        .await?;

    ctx.sync.publish(
        AuthSyncEntity::Group,
        AuthSyncAction::Create,
        group.id,
        Audience::perm::<GroupsRead>(),
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
    Extension(ctx): Extension<AuthContext>,
    Path(group_id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateGroupRequest>,
) -> ApiResult<Json<Group>> {
    // Same field gates as create — this path had NONE, so an over-long or
    // NUL-bearing name reached Postgres as a generic 500 and a blank name was
    // silently stored.
    if let Some(ref name) = request.name {
        validate_group_name(name)?;
    }
    if let Some(ref description) = request.description {
        reject_nul(description, "Group description")?;
    }

    // Check if group exists
    let existing_group = ctx.group()
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
        && let Some(existing) = ctx.group().get_by_name(name).await?
            && existing.id != group_id {
                return Err(AppError::conflict("Group name").into());
            }

    // Update group
    let group = ctx.group()
        .update(
            group_id,
            request.name,
            request.description,
            request.permissions,
            request.is_active,
        )
        .await?;

    ctx.sync.publish(
        AuthSyncEntity::Group,
        AuthSyncAction::Update,
        group.id,
        Audience::perm::<GroupsRead>(),
        origin.0,
    );
    // Editing a group's permissions changes the effective permissions of
    // EVERY member, so fan a permissions-changed signal out to each (Owner-
    // scoped) — their devices re-bootstrap /auth/me immediately rather than
    // waiting up to 60s for the per-connection re-check. Batched into a single
    // registry-lock acquisition (the default Users group can contain every
    // user). Best-effort: a query failure just falls back to the re-check.
    if let Ok(member_ids) = ctx.group().get_member_ids(group.id).await {
        ctx.sync.publish_session_to_users(&member_ids, origin.0);
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
    Extension(ctx): Extension<AuthContext>,
    Path(group_id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Check if group exists
    let group = ctx.group()
        .get_by_id(group_id)
        .await?
        .ok_or_else(|| AppError::not_found("Group"))?;

    // Prevent deletion of system groups
    if group.is_system {
        return Err(AppError::bad_request("SYSTEM_GROUP", "Cannot delete system groups").into());
    }

    // Delete group
    ctx.group().delete(group_id).await?;

    ctx.sync.publish(
        AuthSyncEntity::Group,
        AuthSyncAction::Delete,
        group_id,
        Audience::perm::<GroupsRead>(),
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
    auth: RequirePermissions<(GroupsRead,)>,
    Extension(ctx): Extension<AuthContext>,
    Path(group_id): Path<Uuid>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<UserListResponse>> {
    // Check if group exists
    if ctx.group().get_by_id(group_id).await?.is_none() {
        return Err(AppError::not_found("Group").into());
    }

    // Get group members
    let (mut users, total) = ctx.group()
        .get_members(group_id, params.page, params.per_page)
        .await?;

    // Zero out PII (email / last_login_at) for non-admins, consistent with
    // list_users. The repo already returns an empty permissions array, but
    // email + last_login_at would otherwise leak to any GroupsRead holder.
    if !auth.user.is_admin {
        for u in users.iter_mut() {
            u.email = String::new();
            u.permissions = Vec::new();
            u.last_login_at = None;
        }
    }

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
    Extension(ctx): Extension<AuthContext>,
    origin: SyncOrigin,
    Json(request): Json<AssignUserToGroupRequest>,
) -> ApiResult<StatusCode> {
    // Check if user exists
    if ctx.user().get_by_id(request.user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Check if group exists
    let target_group = ctx.group()
        .get_by_id(request.group_id)
        .await?
        .ok_or_else(|| AppError::not_found("Group"))?;

    // Prevent privilege escalation via group assignment: a non-admin holding
    // the delegable groups::assign_users must not be able to add anyone (incl.
    // themselves) to a privileged group — the pre-existing Administrators group
    // (permissions '*') or a group minted with wildcard perms. Require is_admin
    // to assign into any system group, and require the caller to already hold
    // every permission the target group grants (admins bypass). Mirrors the
    // create_group/update_group self-escalation guards (02-permissions F-02).
    if !auth.user.is_admin {
        if target_group.is_system {
            return Err(AppError::forbidden(
                "CANNOT_ASSIGN_PRIVILEGED_GROUP",
                "Only administrators can assign users to system groups",
            )
            .into());
        }
        for perm in &target_group.permissions {
            if !crate::modules::permissions::checker::check_permission_union(
                &auth.user,
                &auth.groups,
                perm,
            ) {
                return Err(AppError::forbidden(
                    "CANNOT_ASSIGN_PRIVILEGED_GROUP",
                    format!(
                        "Cannot assign users to a group granting permission '{}' that you do not hold yourself",
                        perm
                    ),
                )
                .into());
            }
        }
    }

    // Assign user to group
    ctx.user()
        .assign_to_group(request.user_id, request.group_id, Some(auth.user.id))
        .await?;

    // Signal the affected user that their permissions changed so their
    // open sessions re-bootstrap /auth/me immediately (the 60s re-check is
    // the backstop). Owner-scoped to that user only.
    ctx.sync.publish(
        AuthSyncEntity::Session,
        AuthSyncAction::Update,
        request.user_id,
        Audience::owner(request.user_id),
        origin.0,
    );

    // The group's member list changed → refresh admins viewing it elsewhere.
    ctx.sync.publish(
        AuthSyncEntity::Group,
        AuthSyncAction::Update,
        request.group_id,
        Audience::perm::<GroupsRead>(),
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
    Extension(ctx): Extension<AuthContext>,
    Path((user_id, group_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Check if user exists
    if ctx.user().get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Check if group exists
    if ctx.group().get_by_id(group_id).await?.is_none() {
        return Err(AppError::not_found("Group").into());
    }

    // Remove user from group
    ctx.user().remove_from_group(user_id, group_id).await?;

    // Signal the affected user that their permissions changed (Owner-scoped).
    ctx.sync.publish(
        AuthSyncEntity::Session,
        AuthSyncAction::Update,
        user_id,
        Audience::owner(user_id),
        origin.0,
    );

    // The group's member list changed → refresh admins viewing it elsewhere.
    ctx.sync.publish(
        AuthSyncEntity::Group,
        AuthSyncAction::Update,
        group_id,
        Audience::perm::<GroupsRead>(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_name_accepts_ordinary_names() {
        for n in ["Users", "Administrators", "Lab A — imaging", "研究チーム"] {
            assert!(validate_group_name(n).is_ok(), "input {n:?}");
        }
        // Exactly at the varchar(100) bound is legal.
        assert!(validate_group_name(&"g".repeat(GROUP_MAX_NAME_CHARS)).is_ok());
    }

    #[test]
    fn group_name_rejects_blank_over_long_and_control_bearing() {
        for n in ["", "   ", "bad\u{0}name", "line\nbreak", "spoof\u{202e}ed"] {
            let err = validate_group_name(n).expect_err("expected rejection");
            assert_eq!(err.status_code(), 400, "input {n:?}");
            assert_eq!(err.error_code(), "VALIDATION_ERROR", "input {n:?}");
        }
        let err = validate_group_name(&"g".repeat(GROUP_MAX_NAME_CHARS + 1))
            .expect_err("over-cap name must be rejected");
        assert_eq!(err.status_code(), 400);
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
    }

    #[test]
    fn group_name_bound_counts_characters_not_bytes() {
        // `groups.name` is varchar(100) and Postgres bounds varchar by
        // CHARACTERS, so 100 multi-byte characters must be accepted.
        assert!(validate_group_name(&"é".repeat(GROUP_MAX_NAME_CHARS)).is_ok());
    }

    /// INV-3 — this module's wrapper must render the SHARED message format.
    /// The status/error-code pair is NOT sufficient: every hand-rolled
    /// `AppError::bad_request("VALIDATION_ERROR", …)` produces it, including
    /// the private copy this wrapper replaced. The message is what a re-fork
    /// changes.
    #[test]
    fn reject_nul_wrapper_renders_the_shared_message_format() {
        let err = reject_nul("a\0b", "Group description").expect_err("rejects");
        assert_eq!(err.status_code(), 400);
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
        let rendered = serde_json::to_string(&err).expect("serialize");
        assert!(
            rendered.contains("Group description cannot contain NUL characters"),
            "wrapper drifted from the shared message format: {rendered}"
        );
    }

    #[test]
    fn description_rejects_only_nul() {
        assert!(reject_nul("multi\nline\tprose", "Group description").is_ok());
        let err = reject_nul("bad\u{0}description", "Group description")
            .expect_err("expected rejection");
        assert_eq!(err.status_code(), 400);
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
    }
}
