// User handlers and request/response models

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    core::EventBus,
    modules::permissions::{RequirePermissions, with_permission},
};

use crate::modules::user::{
    events::UserEvent,
    models::User,
    permissions::*,
    types::{CreateUserRequest, ResetPasswordRequest, UpdateUserRequest, UserActiveStatusResponse, UserListResponse},
};

// =====================================================
// Route Handlers
// =====================================================

/// List all users (requires users::read permission)
#[debug_handler]
pub async fn list_users(
    _auth: RequirePermissions<(UsersRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<UserListResponse>> {
    let (users, total) = Repos.user.list(params.page, params.per_page).await?;

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

/// Documentation for list_users endpoint
pub fn list_users_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersRead,)>(op)
        .id("User.list")
        .tag("Users")
        .summary("List all users with pagination")
        .response::<200, Json<UserListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get user by ID (requires users::read permission)
#[debug_handler]
pub async fn get_user(
    _auth: RequirePermissions<(UsersRead,)>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    Ok((StatusCode::OK, Json(user)))
}

/// Documentation for get_user endpoint
pub fn get_user_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersRead,)>(op)
        .id("User.get")
        .tag("Users")
        .summary("Get user by ID")
        .response::<200, Json<User>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Create a new user (requires users::create permission)
#[debug_handler]
pub async fn create_user(
    auth: RequirePermissions<(UsersCreate,)>,

    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateUserRequest>,
) -> ApiResult<Json<User>> {
    // Validate username and email format
    if request.username.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Username cannot be empty").into());
    }
    if request.email.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Email cannot be empty").into());
    }

    // Prevent self-escalation via the permissions field: every permission
    // the caller is trying to grant must be one the caller themselves
    // holds (via user perms OR group union). Admins (is_admin=true)
    // bypass. Without this, any users::create holder can mint a wildcard
    // root by posting {"permissions": ["*"]} — 03-user F-04 (High).
    if let Some(ref requested_perms) = request.permissions {
        if !auth.user.is_admin {
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
    }

    // Check if username already exists
    if Repos
        .user
        .get_by_username(&request.username)
        .await?
        .is_some()
    {
        return Err(AppError::conflict("Username").into());
    }

    // Check if email already exists
    if Repos.user.get_by_email(&request.email).await?.is_some() {
        return Err(AppError::conflict("Email").into());
    }

    // Hash password
    let password_hash = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::internal_error(format!("Failed to hash password: {}", e)))?;

    // Create user
    let user = Repos
        .user
        .create(
            &request.username,
            &request.email,
            Some(password_hash),
            request.display_name,
            request.permissions,
        )
        .await?;

    // Assign user to default group if it exists
    if let Some(default_group) = Repos.group.get_default().await? {
        // Assign user to default group (assigned_by is None for automatic assignment)
        let _ = Repos
            .user
            .assign_to_group(user.id, default_group.id, None)
            .await;
        // Note: We ignore errors here to not fail user creation if group assignment fails
    }

    // Emit UserCreated event asynchronously
    event_bus.emit_async(UserEvent::created(user.clone()));

    Ok((StatusCode::CREATED, Json(user)))
}

/// Documentation for create_user endpoint
pub fn create_user_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersCreate,)>(op)
        .id("User.create")
        .tag("Users")
        .summary("Create a new user account")
        .response::<201, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update user (requires users::edit permission)
#[debug_handler]
pub async fn update_user(
    _auth: RequirePermissions<(UsersEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<User>> {
    // Check if user exists and get user data
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    // Prevent disabling admin users
    if user.is_admin && request.is_active == Some(false) {
        return Err(
            AppError::bad_request("CANNOT_DISABLE_ADMIN", "Cannot disable admin users").into(),
        );
    }

    // Check if new username already exists
    if let Some(ref username) = request.username {
        if let Some(existing) = Repos.user.get_by_username(username).await? {
            if existing.id != user_id {
                return Err(AppError::conflict("Username").into());
            }
        }
    }

    // Check if new email already exists
    if let Some(ref email) = request.email {
        if let Some(existing) = Repos.user.get_by_email(email).await? {
            if existing.id != user_id {
                return Err(AppError::conflict("Email").into());
            }
        }
    }

    // Update user.
    //
    // permissions is intentionally None here — see note on UpdateUserRequest
    // in modules/user/types.rs. Removing the field from the DTO at the
    // serde layer closes 03-user F-01 (Critical priv-esc); passing None
    // through keeps the repository signature stable for the
    // future dedicated set_permissions endpoint.
    Repos
        .user
        .update(
            user_id,
            request.username,
            request.email,
            request.display_name,
            None,
        )
        .await?;

    // Update active status if provided
    if let Some(is_active) = request.is_active {
        Repos.user.set_active(user_id, is_active).await?;
    }

    // Fetch updated user
    let updated_user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    // Emit update event for other modules to react
    event_bus.emit_async(UserEvent::updated(updated_user.clone()));

    Ok((StatusCode::OK, Json(updated_user)))
}

/// Documentation for update_user endpoint
pub fn update_user_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersEdit,)>(op)
        .id("User.update")
        .tag("Users")
        .summary("Update user")
        .response::<200, Json<User>>()
        .response_with::<400, (), _>(|res| {
            res.description("Bad request - validation failed or attempting to disable admin user")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Toggle user active status (requires users::toggle_status permission)
#[debug_handler]
pub async fn toggle_user_active(
    _auth: RequirePermissions<(UsersToggleStatus,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<UserActiveStatusResponse>> {
    // Get current user
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    // Prevent disabling admin users
    if user.is_admin && user.is_active {
        return Err(
            AppError::bad_request("CANNOT_DISABLE_ADMIN", "Cannot disable admin users").into(),
        );
    }

    // Toggle active status
    let new_status = !user.is_active;
    Repos.user.set_active(user_id, new_status).await?;

    // Fetch updated user and emit event
    let updated_user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    event_bus.emit_async(UserEvent::updated(updated_user));

    Ok((
        StatusCode::OK,
        Json(UserActiveStatusResponse {
            user_id,
            is_active: new_status,
        }),
    ))
}

/// Documentation for toggle_user_active endpoint
pub fn toggle_user_active_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersToggleStatus,)>(op)
        .id("User.toggleActive")
        .tag("Users")
        .summary("Toggle user active status")
        .response::<200, Json<UserActiveStatusResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description("Bad request - attempting to disable admin user")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Reset user password (requires users::reset_password permission)
#[debug_handler]
pub async fn reset_user_password(
    _auth: RequirePermissions<(UsersResetPassword,)>,

    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {
    // Check if user exists
    if Repos.user.get_by_id(request.user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Hash new password
    let password_hash = bcrypt::hash(&request.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::internal_error(format!("Failed to hash password: {}", e)))?;

    // Update password
    Repos
        .user
        .update_password(request.user_id, &password_hash)
        .await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Documentation for reset_user_password endpoint
pub fn reset_user_password_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersResetPassword,)>(op)
        .id("User.resetPassword")
        .tag("Users")
        .summary("Reset user password")
        .response_with::<204, (), _>(|res| res.description("Password reset successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Delete user (requires users::delete permission)
#[debug_handler]
pub async fn delete_user(
    _auth: RequirePermissions<(UsersDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Check if user exists
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    // Prevent deleting admin users (symmetric with toggle_user_active and
    // update_user). Without this guard, a users::delete grant lets a
    // sub-admin delete the root admin and brick the deployment — the
    // unique_root_admin partial index then prevents re-creation.
    // Closes 03-user F-02 (Critical).
    if user.is_admin {
        return Err(
            AppError::bad_request("CANNOT_DELETE_ADMIN", "Cannot delete admin users").into(),
        );
    }

    // Delete user
    Repos.user.delete(user_id).await?;

    // Emit deletion event for other modules to react
    event_bus.emit_async(UserEvent::deleted(user_id));

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Documentation for delete_user endpoint
pub fn delete_user_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(UsersDelete,)>(op)
        .id("User.delete")
        .tag("Users")
        .summary("Delete user")
        .response_with::<204, (), _>(|res| res.description("User deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

