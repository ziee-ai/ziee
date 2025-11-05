// Auth routes configuration

use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
use axum::{routing::get, Json};
use sqlx::PgPool;

use super::handlers::*;
use super::jwt::TokenPair;
use super::types::{AuthResponse, MeResponse};

/// Auth routes configuration
pub fn auth_routes() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route("/register", post_with(register, register_docs))
        .api_route("/login", post_with(login, login_docs))
        .api_route("/refresh", post_with(refresh, refresh_docs))
        .api_route("/logout", post_with(logout, logout_docs))
        .api_route("/me", get_with(me, me_docs))
        // OAuth routes use regular routing (not aide) since they return redirects
        .route("/oauth/{provider_name}/authorize", get(oauth_authorize))
        .route("/oauth/{provider_name}/callback", get(oauth_callback))
}
