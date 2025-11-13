// User routes configuration

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
};

use super::handlers::*;

/// User management routes
pub fn user_router() -> ApiRouter {
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
        .api_route(
            "/users/{user_id}",
            delete_with(delete_user, delete_user_docs),
        )
}

/// Group management routes
pub fn group_router() -> ApiRouter {
    ApiRouter::new()
        .api_route("/groups", get_with(list_groups, list_groups_docs))
        .api_route("/groups", post_with(create_group, create_group_docs))
        .api_route("/groups/{group_id}", get_with(get_group, get_group_docs))
        .api_route(
            "/groups/{group_id}",
            post_with(update_group, update_group_docs),
        )
        .api_route(
            "/groups/{group_id}",
            delete_with(delete_group, delete_group_docs),
        )
        .api_route(
            "/groups/{group_id}/members",
            get_with(get_group_members, get_group_members_docs),
        )
        .api_route(
            "/groups/assign",
            post_with(assign_user_to_group, assign_user_to_group_docs),
        )
        .api_route(
            "/groups/{user_id}/{group_id}/remove",
            delete_with(remove_user_from_group, remove_user_from_group_docs),
        )
}
