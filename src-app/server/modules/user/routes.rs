// User routes configuration

use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
use sqlx::PgPool;

use super::handlers::*;

/// User management routes
pub fn user_router() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route("/users", get_with(list_users, list_users_docs))
        .api_route("/users", post_with(create_user, create_user_docs))
        .api_route("/users/{user_id}", get_with(get_user, get_user_docs))
        .api_route("/users/{user_id}", post_with(update_user, update_user_docs))
        .api_route("/users/{user_id}/toggle-active", post_with(toggle_user_active, toggle_user_active_docs))
        .api_route("/users/reset-password", post_with(reset_user_password, reset_user_password_docs))
        .api_route("/users/{user_id}", aide::axum::routing::delete_with(delete_user, delete_user_docs))
}
