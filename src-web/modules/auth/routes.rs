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
        .api_route(
            "/register",
            post_with(register, |op| {
                op.description("Register a new user with username, email, and password")
                    .id("Auth.register")
                    .tag("auth")
                    .response::<201, Json<AuthResponse>>()
            }),
        )
        .api_route(
            "/login",
            post_with(login, |op| {
                op.description("Login with username/email and password")
                    .id("Auth.login")
                    .tag("auth")
                    .response::<200, Json<AuthResponse>>()
            }),
        )
        .api_route(
            "/refresh",
            post_with(refresh, |op| {
                op.description("Refresh access token using refresh token")
                    .id("Auth.refresh")
                    .tag("auth")
                    .response::<200, Json<TokenPair>>()
            }),
        )
        .api_route(
            "/logout",
            post_with(logout, |op| {
                op.description("Logout current user")
                    .id("Auth.logout")
                    .tag("auth")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/me",
            get_with(me, |op| {
                op.description("Get currently authenticated user with their effective permissions")
                    .id("Auth.me")
                    .tag("auth")
                    .response::<200, Json<MeResponse>>()
            }),
        )
        // OAuth routes use regular routing (not aide) since they return redirects
        .route("/oauth/{provider_name}/authorize", get(oauth_authorize))
        .route("/oauth/{provider_name}/callback", get(oauth_callback))
}
