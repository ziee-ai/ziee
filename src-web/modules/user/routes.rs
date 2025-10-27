use super::models::*;
use super::service::UserService;
use crate::common::{ApiResult, AppError, PaginationQuery};
use aide::axum::{
    routing::{delete_with, get_with, post_with, put_with},
    ApiRouter,
};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct UserState {
    pub service: Arc<UserService>,
}

pub fn routes(state: UserState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/users",
            post_with(create_user, |op| {
                op.description("Create a new user")
                    .id("User.create")
                    .tag("users")
                    .response::<200, Json<User>>()
            }),
        )
        .api_route(
            "/users",
            get_with(list_users, |op| {
                op.description("List users with pagination")
                    .id("User.list")
                    .tag("users")
                    .response::<200, Json<UserListResponse>>()
            }),
        )
        .api_route(
            "/users/{id}",
            get_with(get_user, |op| {
                op.description("Get user by ID")
                    .id("User.get")
                    .tag("users")
                    .response::<200, Json<User>>()
            }),
        )
        .api_route(
            "/users/{id}",
            put_with(update_user, |op| {
                op.description("Update user")
                    .id("User.update")
                    .tag("users")
                    .response::<200, Json<User>>()
            }),
        )
        .api_route(
            "/users/{id}",
            delete_with(delete_user, |op| {
                op.description("Delete user")
                    .id("User.delete")
                    .tag("users")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/users/{id}/password",
            post_with(change_password, |op| {
                op.description("Change user password")
                    .id("User.changePassword")
                    .tag("users")
                    .response::<200, Json<serde_json::Value>>()
            }),
        )
        .api_route(
            "/users/{id}/reset-password",
            post_with(reset_password, |op| {
                op.description("Reset user password (admin)")
                    .id("User.resetPassword")
                    .tag("users")
                    .response::<200, Json<serde_json::Value>>()
            }),
        )
        .with_state(state)
}

async fn create_user(
    State(state): State<UserState>,
    Json(request): Json<CreateUserRequest>,
) -> ApiResult<Json<User>> {
    let user = state.service.create_user(request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(user.sanitized())))
}

async fn get_user(
    State(state): State<UserState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<User>> {
    let user = state
        .service
        .get_user(id)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("User").to_api_error())?;
    Ok((StatusCode::OK, Json(user.sanitized())))
}

async fn list_users(
    State(state): State<UserState>,
    Query(pagination): Query<PaginationQuery>,
) -> ApiResult<Json<UserListResponse>> {
    let response = state
        .service
        .list_users(pagination.page, pagination.per_page)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?;

    // Sanitize all users
    let sanitized_response = UserListResponse {
        users: response.users.into_iter().map(|u| u.sanitized()).collect(),
        total: response.total,
        page: response.page,
        per_page: response.per_page,
    };

    Ok((StatusCode::OK, Json(sanitized_response)))
}

async fn update_user(
    State(state): State<UserState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<User>> {
    let user = state
        .service
        .update_user(id, request)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("User").to_api_error())?;
    Ok((StatusCode::OK, Json(user.sanitized())))
}

async fn delete_user(
    State(state): State<UserState>,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    state
        .service
        .delete_user(id)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::NO_CONTENT, ()))
}

async fn change_password(
    State(state): State<UserState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ChangePasswordRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    state.service.change_password(id, request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Password changed successfully"}))))
}

async fn reset_password(
    State(state): State<UserState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ResetPasswordRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    state.service.reset_password(id, request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Password reset successfully"}))))
}
