//! Routes for tunnel-aware auth endpoints.
//!
//! These mount at `/api/auth/config` and `/api/auth/login-password-only`
//! to match the URL convention of the rest of the auth surface.
//! Both are UNAUTHENTICATED.

use ziee::{ApiRouter, get_with, post_with};

use super::handlers::{
    change_password, change_password_docs, get_auth_config, get_auth_config_docs,
    login_password_only, login_password_only_docs,
};

pub fn tunnel_auth_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route("/api/auth/config", get_with(get_auth_config, get_auth_config_docs))
        .api_route(
            "/api/auth/login-password-only",
            post_with(login_password_only, login_password_only_docs),
        )
        .api_route(
            "/api/users/me/password",
            post_with(change_password, change_password_docs),
        )
}
