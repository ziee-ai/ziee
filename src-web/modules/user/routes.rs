use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
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
        CreateUserRequest, ResetPasswordRequest, UpdateUserRequest,
        UserActiveStatusResponse, UserListResponse, User,
    },
    permissions::*,
    repository::UserRepository,
};

/// User management routes
pub fn user_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route("/users", get_with(list_users, list_users_docs))
        .api_route("/users", post_with(create_user, create_user_docs))
        .api_route("/users/{user_id}", get_with(get_user, get_user_docs))
        .api_route("/users/{user_id}", post_with(update_user, update_user_docs))
        .api_route(
            "/users/{user_id}/toggle-active",
            post_with(toggle_user_active, toggle_user_active_docs),
        )
        .api_route(
            "/users/reset-password",
            post_with(reset_user_password, reset_user_password_docs),
        )
        .api_route("/users/{user_id}", aide::axum::routing::delete_with(delete_user, delete_user_docs))
}

// =====================================================
// Route Handlers
// =====================================================

/// List all users (requires users::read permission)
async fn list_users(
    _auth: RequirePermissions<(UsersRead,)>,
    Query(params): Query<PaginationQuery>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<UserListResponse>> {
    let user_repo = UserRepository::new(pool);
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

fn list_users_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersRead,)>(op)
        .id("User.list")
        .tag("Users")
        .summary("List all users with pagination")
        .response::<200, Json<UserListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get user by ID (requires users::read permission)
async fn get_user(
    _auth: RequirePermissions<(UsersRead,)>,
    Path(user_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<User>> {
    let user_repo = UserRepository::new(pool);
    let user = user_repo
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;

    Ok((StatusCode::OK, Json(user)))
}

fn get_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersRead,)>(op)
        .id("User.get")
        .tag("Users")
        .summary("Get user by ID")
        .response::<200, Json<User>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Create a new user (requires users::create permission)
async fn create_user(
    _auth: RequirePermissions<(UsersCreate,)>,
    State(pool): State<PgPool>,
    Json(request): Json<CreateUserRequest>,
) -> ApiResult<Json<User>> {
    // Validate username and email format
    if request.username.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Username cannot be empty").into());
    }
    if request.email.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Email cannot be empty").into());
    }

    let user_repo = UserRepository::new(pool);

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

fn create_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersCreate,)>(op)
        .id("User.create")
        .tag("Users")
        .summary("Create a new user account")
        .response::<201, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update user (requires users::edit permission)
async fn update_user(
    _auth: RequirePermissions<(UsersEdit,)>,
    Path(user_id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<User>> {
    let user_repo = UserRepository::new(pool);

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
    let user = user_repo
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

fn update_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersEdit,)>(op)
        .id("User.update")
        .tag("Users")
        .summary("Update user")
        .response::<200, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Toggle user active status (requires users::toggle-status permission)
async fn toggle_user_active(
    _auth: RequirePermissions<(UsersToggleStatus,)>,
    Path(user_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<UserActiveStatusResponse>> {
    let user_repo = UserRepository::new(pool);

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

fn toggle_user_active_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(UsersToggleStatus,)>(op)
        .id("User.toggleActive")
        .tag("Users")
        .summary("Toggle user active status")
        .response::<200, Json<UserActiveStatusResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Reset user password (requires users::reset-password permission)
async fn reset_user_password(
    _auth: RequirePermissions<(UsersResetPassword,)>,
    State(pool): State<PgPool>,
    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {
    let user_repo = UserRepository::new(pool);

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

fn reset_user_password_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(UsersResetPassword,)>(op)
        .id("User.resetPassword")
        .tag("Users")
        .summary("Reset user password")
        .response_with::<204, (), _>(|res| res.description("Password reset successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}

/// Delete user (requires users::delete permission)
async fn delete_user(
    _auth: RequirePermissions<(UsersDelete,)>,
    Path(user_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<StatusCode> {
    let user_repo = UserRepository::new(pool);

    // Check if user exists
    if user_repo.get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }

    // Delete user
    user_repo.delete(user_id).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

fn delete_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersDelete,)>(op)
        .id("User.delete")
        .tag("Users")
        .summary("Delete user")
        .response_with::<204, (), _>(|res| res.description("User deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}
