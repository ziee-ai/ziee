use super::models::*;
use super::service::AuthService;
use crate::common::{ApiResult, AppError};
use crate::modules::auth::{invalid_token, missing_token};
use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthState {
    pub service: Arc<AuthService>,
}

pub fn routes(state: AuthState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/auth/login",
            post_with(login, |op| {
                op.description("Login with username/email and password")
                    .id("Auth.login")
                    .tag("auth")
                    .response::<200, Json<LoginResponse>>()
            }),
        )
        .api_route(
            "/auth/register",
            post_with(register, |op| {
                op.description("Register a new user")
                    .id("Auth.register")
                    .tag("auth")
                    .response::<200, Json<LoginResponse>>()
            }),
        )
        .api_route(
            "/auth/logout",
            post_with(logout, |op| {
                op.description("Logout current user")
                    .id("Auth.logout")
                    .tag("auth")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/auth/me",
            get_with(get_current_user, |op| {
                op.description("Get current user information")
                    .id("Auth.getCurrentUser")
                    .tag("auth")
                    .response::<200, Json<CurrentUserResponse>>()
            }),
        )
        .with_state(state)
}

async fn login(
    State(state): State<AuthState>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    let response = state.service.login(request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(response)))
}

async fn register(
    State(state): State<AuthState>,
    Json(request): Json<RegisterRequest>,
) -> ApiResult<Json<LoginResponse>> {
    let response = state.service.register(request).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    Ok((StatusCode::OK, Json(response)))
}

async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> ApiResult<()> {
    // Extract token from Authorization header
    let token = extract_token(&headers).ok_or_else(missing_token)?;

    let result = state.service.logout(&token).await
        .map_err(|e| AppError::from(e).to_api_error())?;
    if !result {
        return Err(invalid_token().to_api_error());
    }

    Ok((StatusCode::NO_CONTENT, ()))
}

async fn get_current_user(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> ApiResult<Json<CurrentUserResponse>> {
    // Extract token from Authorization header
    let token = extract_token(&headers).ok_or_else(missing_token)?;

    let user = state
        .service
        .get_current_user(&token)
        .await
        .map_err(|e| AppError::from(e).to_api_error())?
        .ok_or_else(invalid_token)?;

    Ok((StatusCode::OK, Json(CurrentUserResponse {
        user: user.sanitized(),
    })))
}

/// Extract bearer token from Authorization header
fn extract_token(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get(header::AUTHORIZATION)?.to_str().ok()?;

    if auth_header.starts_with("Bearer ") {
        Some(auth_header[7..].to_string())
    } else {
        None
    }
}
