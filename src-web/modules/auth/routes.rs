use aide::axum::{
    routing::{get_with, post_with},
    ApiRouter,
};
use axum::{debug_handler, extract::{Path, Query, State}, http::StatusCode, response::{IntoResponse, Redirect}, routing::get, Extension, Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::modules::user::{User, UserRepository, GroupRepository, UserService};

use super::jwt::{JwtService, TokenPair};
use super::jwt_extractor::JwtAuth;
use super::password;
use super::providers::{create_provider, repository as provider_repo};

// =====================================================
// Request/Response Models
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Optional provider name for LDAP/OAuth authentication
    /// If not specified, defaults to local password authentication
    pub provider: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AuthResponse {
    pub user: User,
    #[serde(flatten)]
    pub tokens: TokenPair,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthAuthorizeQuery {
    /// Redirect URI after successful authentication
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthCallbackQuery {
    /// Authorization code from provider
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MeResponse {
    pub user: User,
    /// Effective permissions (union of user's direct permissions + all active group permissions)
    pub permissions: Vec<String>,
}

// =====================================================
// Route Handlers
// =====================================================

/// POST /api/auth/register
/// Register a new user with username, email, and password
#[debug_handler]
pub async fn register(
    State(pool): State<PgPool>,
    Extension(jwt_service): Extension<Arc<JwtService>>,
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

    let user_repo = UserRepository::new(pool);

    // Hash password
    let password_hash = password::hash_password(&req.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Failed to hash password: {}", e))))?;

    // Create user
    let user = user_repo
        .create(&req.username, &req.email, Some(password_hash), req.display_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::CREATED, Json(AuthResponse {
        user,
        tokens,
    })))
}

/// POST /api/auth/login
/// Login with username/email and password
#[debug_handler]
pub async fn login(
    State(pool): State<PgPool>,
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let user_repo = UserRepository::new(pool.clone());

    // Check if external provider is specified
    if let Some(provider_name) = &req.provider {
        if provider_name != "local" {
            // External authentication (LDAP/OAuth)
            return login_with_provider(pool, jwt_service, &req.username, &req.password, provider_name).await;
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
    let provider_config = provider_repo::get_provider_by_name(&pool, provider_name)
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
        .map_err(|e| (StatusCode::UNAUTHORIZED, AppError::unauthorized("INVALID_CREDENTIALS", format!("Invalid username or password"))))?;

    // Try to find user via auth link
    let link = sqlx::query!(
        r#"
        SELECT user_id
        FROM user_auth_links
        WHERE provider_id = $1 AND external_id = $2
        "#,
        provider_config.id,
        auth_result.external_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    let user_repo = UserRepository::new(pool.clone());

    let user = if let Some(link) = link {
        // User exists, get it
        user_repo.get_by_id(link.user_id).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?
    } else {
        // User doesn't exist - create new user
        let new_user_id = uuid::Uuid::new_v4();
        let display_name = auth_result.attributes.display_name.unwrap_or_else(|| username.to_string());
        let email = auth_result.attributes.email;

        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, display_name, is_active, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())
            "#,
            new_user_id,
            username,
            email,
            display_name
        )
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

        // Create auth link
        sqlx::query!(
            r#"
            INSERT INTO user_auth_links (user_id, provider_id, external_id, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
            new_user_id,
            provider_config.id,
            auth_result.external_id
        )
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

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
    State(pool): State<PgPool>,
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
    let user_repo = UserRepository::new(pool);
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

/// POST /api/auth/logout
/// Logout current user (JWT is stateless, so this is just a placeholder)
/// Client should discard the token
#[debug_handler]
pub async fn logout(_auth: JwtAuth) -> ApiResult<()> {
    // JWT is stateless, logout is handled client-side by discarding the token
    // This endpoint exists for API consistency
    Ok((StatusCode::NO_CONTENT, ()))
}

/// GET /api/auth/me
/// Get currently authenticated user with their effective permissions
#[debug_handler]
pub async fn me(
    State(pool): State<PgPool>,
    auth: JwtAuth,
) -> ApiResult<Json<MeResponse>> {
    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Invalid user ID in token: {}", e))))?;

    // Get user from database
    let user_repo = UserRepository::new(pool.clone());
    let user = user_repo
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

    // Get effective permissions (union of user permissions + group permissions)
    let group_repo = GroupRepository::new(pool);
    let user_service = UserService::new(user_repo, group_repo);
    let permissions = user_service
        .get_effective_permissions(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(MeResponse { user, permissions })))
}

/// GET /api/auth/oauth/{provider_name}/authorize
/// Initiate OAuth flow for the specified provider
#[debug_handler]
pub async fn oauth_authorize(
    State(pool): State<PgPool>,
    Path(provider_name): Path<String>,
    Query(query): Query<OAuthAuthorizeQuery>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // Get provider configuration
    let provider_config = provider_repo::get_provider_by_name(&pool, &provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Database error: {}", e))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Authentication provider")))?;

    // Create provider instance
    let provider = create_provider(&provider_config, pool)
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
    State(pool): State<PgPool>,
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Path(provider_name): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // Get provider configuration
    let provider_config = provider_repo::get_provider_by_name(&pool, &provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Database error: {}", e))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Authentication provider")))?;

    // Create provider instance
    let provider = create_provider(&provider_config, pool.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e))))?;

    // Handle OAuth callback
    let auth_result = provider
        .handle_oauth_callback(&query.code, &query.state, &query.state)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, AppError::unauthorized("OAUTH_FAILED", format!("OAuth authentication failed: {}", e))))?;

    // Try to find user via auth link
    let link = sqlx::query!  (
        r#"
        SELECT user_id
        FROM user_auth_links
        WHERE provider_id = $1 AND external_id = $2
        "#,
        provider_config.id,
        auth_result.external_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    if let Some(link) = link {
        let user_repo = UserRepository::new(pool);
        let user = user_repo.get_by_id(link.user_id).await
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

// =====================================================
// Router Setup
// =====================================================

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
                op.description("Get currently authenticated user")
                    .id("Auth.me")
                    .tag("auth")
                    .response::<200, Json<User>>()
            }),
        )
        // OAuth routes use regular routing (not aide) since they return redirects
        .route("/oauth/{provider_name}/authorize", get(oauth_authorize))
        .route("/oauth/{provider_name}/callback", get(oauth_callback))
}
