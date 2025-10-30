// User handlers and request/response models

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
    models::User,
    types::{
        CreateUserRequest, ResetPasswordRequest, UpdateUserRequest,
        UserActiveStatusResponse, UserListResponse,
    },
    permissions::*,
    repository::UserRepository,
};

// =====================================================
// Route Handlers
// =====================================================

/// List all users (requires users::read permission)
pub async fn list_users(
    _auth: RequirePermissions<(UsersRead,)>,
    Query(params): Query<PaginationQuery>,
    Extension(user_repo): Extension<UserRepository>,
) -> ApiResult<Json<UserListResponse>> {
    let (users, total) = user_repo.list(params.page, params.per_page).await?;

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

/// Get user by ID (requires users::read permission)
pub async fn get_user(
    _auth: RequirePermissions<(UsersRead,)>,
    Path(user_id): Path<Uuid>,
    Extension(user_repo): Extension<UserRepository>,
) -> ApiResult<Json<User>> {
    let user = user_repo
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    Ok((StatusCode::OK, Json(user)))
}

/// Create a new user (requires users::create permission)
pub async fn create_user(
    _auth: RequirePermissions<(UsersCreate,)>,
    Extension(user_repo): Extension<UserRepository>,
    Json(request): Json<CreateUserRequest>,
) -> ApiResult<Json<User>> {
    // Validate username and email format
    if request.username.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Username cannot be empty").into());
    }
    if request.email.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Email cannot be empty").into());
    }

    // Check if username already exists
    if user_repo.get_by_username(&request.username).await?.is_some() {
        return Err(AppError::conflict("Username").into());
    }

    // Check if email already exists
    if user_repo.get_by_email(&request.email).await?.is_some() {
        return Err(AppError::conflict("Email").into());
    }

    // Hash password
    let password_hash = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::internal_error(format!("Failed to hash password: {}", e)))?;

    // Create user
    let user = user_repo
        .create(
            &request.username,
            &request.email,
            Some(password_hash),
            request.display_name,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

/// Update user (requires users::edit permission)
pub async fn update_user(
    _auth: RequirePermissions<(UsersEdit,)>,
    Path(user_id): Path<Uuid>,
    Extension(user_repo): Extension<UserRepository>,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<User>> {

    // Check if user exists
    if user_repo.get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Check if new username already exists
    if let Some(ref username) = request.username {
        if let Some(existing) = user_repo.get_by_username(username).await? {
            if existing.id != user_id {
                return Err(AppError::conflict("Username").into());
            }
        }
    }

    // Check if new email already exists
    if let Some(ref email) = request.email {
        if let Some(existing) = user_repo.get_by_email(email).await? {
            if existing.id != user_id {
                return Err(AppError::conflict("Email").into());
            }
        }
    }

    // Update user
    user_repo
        .update(user_id, request.username, request.email, request.display_name)
        .await?;

    // Update active status if provided
    if let Some(is_active) = request.is_active {
        user_repo.set_active(user_id, is_active).await?;
    }

    // Fetch updated user
    let updated_user = user_repo
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    Ok((StatusCode::OK, Json(updated_user)))
}

/// Toggle user active status (requires users::toggle-status permission)
pub async fn toggle_user_active(
    _auth: RequirePermissions<(UsersToggleStatus,)>,
    Path(user_id): Path<Uuid>,
    Extension(user_repo): Extension<UserRepository>,
) -> ApiResult<Json<UserActiveStatusResponse>> {

    // Get current user
    let user = user_repo
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    // Toggle active status
    let new_status = !user.is_active;
    user_repo.set_active(user_id, new_status).await?;

    Ok((
        StatusCode::OK,
        Json(UserActiveStatusResponse {
            user_id,
            is_active: new_status,
        }),
    ))
}

/// Reset user password (requires users::reset-password permission)
pub async fn reset_user_password(
    _auth: RequirePermissions<(UsersResetPassword,)>,
    Extension(user_repo): Extension<UserRepository>,
    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {

    // Check if user exists
    if user_repo.get_by_id(request.user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Hash new password
    let password_hash = bcrypt::hash(&request.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::internal_error(format!("Failed to hash password: {}", e)))?;

    // Update password
    user_repo
        .update_password(request.user_id, &password_hash)
        .await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// Delete user (requires users::delete permission)
pub async fn delete_user(
    _auth: RequirePermissions<(UsersDelete,)>,
    Path(user_id): Path<Uuid>,
    Extension(user_repo): Extension<UserRepository>,
) -> ApiResult<StatusCode> {

    // Check if user exists
    if user_repo.get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Delete user
    user_repo.delete(user_id).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}
