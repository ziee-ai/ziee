// User routes configuration

use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
use axum::Json;
use sqlx::PgPool;

use crate::modules::permissions::with_permission;

use super::{
    handlers::*,
    models::User,
    types::{UserActiveStatusResponse, UserListResponse},
    permissions::*,
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
// API Documentation
// =====================================================

fn list_users_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersRead,)>(op)
        .id("User.list")
        .tag("Users")
        .summary("List all users with pagination")
        .response::<200, Json<UserListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
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

fn create_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersCreate,)>(op)
        .id("User.create")
        .tag("Users")
        .summary("Create a new user account")
        .response::<201, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
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

fn delete_user_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(UsersDelete,)>(op)
        .id("User.delete")
        .tag("Users")
        .summary("Delete user")
        .response_with::<204, (), _>(|res| res.description("User deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("User not found"))
}
