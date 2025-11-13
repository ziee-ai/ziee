// Auth handlers

use aide::transform::TransformOperation;
use axum::{debug_handler, extract::{Path, Query}, http::StatusCode, response::{IntoResponse, Redirect}, Extension, Json};
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::user::{UserRepository, GroupRepository, UserService};
use crate::modules::user::events::UserEvent;

use super::jwt::{JwtService, TokenPair};
use super::jwt_extractor::JwtAuth;
use super::password;
use super::providers::{create_provider, repository as provider_repo};
use super::types::{
    RegisterRequest, LoginRequest, AuthResponse, RefreshTokenRequest,
    OAuthAuthorizeQuery, OAuthCallbackQuery, MeResponse,
};

// =====================================================
// Route Handlers
// =====================================================

/// POST /api/auth/register
/// Register a new user with username, email, and password
#[debug_handler]
pub async fn register(
    
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Validate input fields
    if req.username.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, AppError::bad_request("INVALID_USERNAME", "Username cannot be empty")));
    }
    if req.email.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, AppError::bad_request("INVALID_EMAIL", "Email cannot be empty")));
    }
    if req.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, AppError::bad_request("INVALID_PASSWORD", "Password cannot be empty")));
    }

    let user_repo = UserRepository::new(Repos.pool().clone());

    // Hash password
    let password_hash = password::hash_password(&req.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Failed to hash password: {}", e))))?;

    // Create user
    let user = user_repo
        .create(&req.username, &req.email, Some(password_hash), req.display_name, None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Auto-assign user to default group
    Repos.auth
        .assign_user_to_default_group(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Emit UserCreated event asynchronously
    event_bus.emit_async(UserEvent::created(user.clone()));

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::CREATED, Json(AuthResponse {
        user,
        tokens,
    })))
}

/// Documentation for register endpoint
pub fn register_docs(op: TransformOperation) -> TransformOperation {
    op.description("Register a new user with username, email, and password")
        .id("Auth.register")
        .tag("auth")
        .response::<201, Json<AuthResponse>>()
}

/// POST /api/auth/login
/// Login with username/email and password
#[debug_handler]
pub async fn login(
    
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let user_repo = UserRepository::new(Repos.pool().clone());

    // Check if external provider is specified
    if let Some(provider_name) = &req.provider {
        if provider_name != "local" {
            // External authentication (LDAP/OAuth)
            return login_with_provider(Repos.pool().clone(), jwt_service, &req.username, &req.password, provider_name).await;
        }
    }

    // Local password authentication
    // Get user by username or email
    let user = user_repo
        .get_by_username_or_email(&req.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, AppError::unauthorized("INVALID_CREDENTIALS", "Invalid username or password")))?;

    // Check if user is active
    if !user.is_active {
        return Err((StatusCode::UNAUTHORIZED, AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled")));
    }

    // Verify password
    let password_hash = user.password_hash.as_ref()
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, AppError::unauthorized("NO_PASSWORD", "No password set for this user. Please use external authentication.")))?;

    let valid = password::verify_password(&req.password, password_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Password verification error: {}", e))))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, AppError::unauthorized("INVALID_CREDENTIALS", "Invalid username or password")));
    }

    // Update last login
    user_repo.update_last_login(user.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(AuthResponse {
        user,
        tokens,
    })))
}

/// Documentation for login endpoint
pub fn login_docs(op: TransformOperation) -> TransformOperation {
    op.description("Login with username/email and password")
        .id("Auth.login")
        .tag("auth")
        .response::<200, Json<AuthResponse>>()
}

/// Login with external provider (LDAP/OAuth)
async fn login_with_provider(
    pool: PgPool,
    jwt_service: Arc<JwtService>,
    username: &str,
    password: &str,
    provider_name: &str,
) -> ApiResult<Json<AuthResponse>> {
    use crate::modules::auth::providers::{create_provider, repository as provider_repo};

    // Get provider configuration
    let provider_config = provider_repo::get_provider_by_name(Repos.pool(), provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Database error: {}", e))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Authentication provider")))?;

    // Create provider instance
    let provider = create_provider(&provider_config, pool.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e))))?;

    // Authenticate with external provider
    let auth_result = provider
        .authenticate(username, password)
        .await
        .map_err(|_e| (StatusCode::UNAUTHORIZED, AppError::unauthorized("INVALID_CREDENTIALS", format!("Invalid username or password"))))?;

    // Try to find user via auth link
    let user_id = Repos.auth
        .find_user_by_auth_link(provider_config.id, &auth_result.external_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let user_repo = UserRepository::new(Repos.pool().clone());

    let user = if let Some(user_id) = user_id {
        // User exists, get it
        user_repo.get_by_id(user_id).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?
    } else {
        // User doesn't exist - create new user with auth link and default group assignment
        let display_name = auth_result.attributes.display_name.unwrap_or_else(|| username.to_string());
        let email = auth_result.attributes.email;

        let new_user_id = Repos.auth
            .create_external_user_with_link(
                username,
                Some(email),
                &display_name,
                provider_config.id,
                &auth_result.external_id,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        // Fetch the newly created user
        user_repo.get_by_id(new_user_id).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error("Failed to fetch newly created user")))?
    };

    // Check if user is active
    if !user.is_active {
        return Err((StatusCode::UNAUTHORIZED, AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled")));
    }

    // Update last login
    user_repo.update_last_login(user.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(AuthResponse {
        user,
        tokens,
    })))
}

/// POST /api/auth/refresh
/// Refresh access token using refresh token
#[debug_handler]
pub async fn refresh(
    
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<RefreshTokenRequest>,
) -> ApiResult<Json<TokenPair>> {
    // Validate refresh token
    let claims = jwt_service
        .validate_refresh_token(&req.refresh_token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Invalid user ID in token: {}", e))))?;

    // Get user from database
    let user_repo = UserRepository::new(Repos.pool().clone());
    let user = user_repo
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, AppError::unauthorized("USER_NOT_FOUND", "User not found")))?;

    // Check if user is still active
    if !user.is_active {
        return Err((StatusCode::UNAUTHORIZED, AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled")));
    }

    // Generate new tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(tokens)))
}

/// Documentation for refresh endpoint
pub fn refresh_docs(op: TransformOperation) -> TransformOperation {
    op.description("Refresh access token using refresh token")
        .id("Auth.refresh")
        .tag("auth")
        .response::<200, Json<TokenPair>>()
}

/// POST /api/auth/logout
/// Logout current user (JWT is stateless, so this is just a placeholder)
/// Client should discard the token
#[debug_handler]
pub async fn logout(_auth: JwtAuth) -> ApiResult<()> {
    // JWT is stateless, logout is handled client-side by discarding the token
    // This endpoint exists for API consistency
    Ok((StatusCode::NO_CONTENT, ()))
}

/// Documentation for logout endpoint
pub fn logout_docs(op: TransformOperation) -> TransformOperation {
    op.description("Logout current user")
        .id("Auth.logout")
        .tag("auth")
        .response::<204, ()>()
}

/// GET /api/auth/me
/// Get currently authenticated user with their effective permissions
#[debug_handler]
pub async fn me(
    
    auth: JwtAuth,
) -> ApiResult<Json<MeResponse>> {
    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Invalid user ID in token: {}", e))))?;

    // Get user from database
    let user_repo = UserRepository::new(Repos.pool().clone());
    let user = user_repo
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

    // Get effective permissions (union of user permissions + group permissions)
    let group_repo = GroupRepository::new(Repos.pool().clone());
    let user_service = UserService::new(user_repo, group_repo);
    let permissions = user_service
        .get_effective_permissions(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(MeResponse { user, permissions })))
}

/// Documentation for me endpoint
pub fn me_docs(op: TransformOperation) -> TransformOperation {
    op.description("Get currently authenticated user with their effective permissions")
        .id("Auth.me")
        .tag("auth")
        .response::<200, Json<MeResponse>>()
}

/// GET /api/auth/oauth/{provider_name}/authorize
/// Initiate OAuth flow for the specified provider
#[debug_handler]
pub async fn oauth_authorize(
    
    Path(provider_name): Path<String>,
    Query(query): Query<OAuthAuthorizeQuery>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // Get provider configuration
    let provider_config = provider_repo::get_provider_by_name(Repos.pool(), &provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Database error: {}", e))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Authentication provider")))?;

    // Create provider instance
    let provider = create_provider(&provider_config, Repos.pool().clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e))))?;

    // Build callback URL (should be a full URL in production)
    let redirect_uri = query.redirect_uri.unwrap_or_else(|| {
        format!("/api/auth/oauth/{}/callback", provider_name)
    });

    // Initialize OAuth flow
    let oauth_result = provider
        .init_oauth_flow(&redirect_uri)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("OAuth initialization failed: {}", e))))?;

    // Redirect to provider's authorization URL
    Ok(Redirect::temporary(&oauth_result.redirect_url))
}

/// GET /api/auth/oauth/{provider_name}/callback
/// Handle OAuth callback from provider
#[debug_handler]
pub async fn oauth_callback(
    
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Path(provider_name): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // Get provider configuration
    let provider_config = provider_repo::get_provider_by_name(Repos.pool(), &provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Database error: {}", e))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Authentication provider")))?;

    // Create provider instance
    let provider = create_provider(&provider_config, Repos.pool().clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e))))?;

    // Handle OAuth callback
    let auth_result = provider
        .handle_oauth_callback(&query.code, &query.state, &query.state)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, AppError::unauthorized("OAUTH_FAILED", format!("OAuth authentication failed: {}", e))))?;

    // Try to find user via auth link
    let user_id = Repos.auth
        .find_user_by_auth_link(provider_config.id, &auth_result.external_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(link_user_id) = user_id {
        let user_repo = UserRepository::new(Repos.pool().clone());
        let user = user_repo.get_by_id(link_user_id).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

        // Check if user is active
        if !user.is_active {
            return Err((StatusCode::UNAUTHORIZED, AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled")));
        }

        // Update last login
        user_repo.update_last_login(user.id).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        // Generate JWT tokens
        let tokens = jwt_service
            .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        // Redirect to success page with token (in a real app, use a more secure method)
        Ok(Redirect::temporary(&format!("/?token={}", tokens.access_token)))
    } else {
        // User doesn't exist - need to provision
        Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized(
                "USER_NOT_PROVISIONED",
                "User not found. Please contact administrator to provision your account.",
            ),
        ))
    }
}
