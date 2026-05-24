// Auth handlers

use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::user::events::UserEvent;
use crate::modules::user::UserService;

use super::jwt::{JwtService, TokenPair};
use super::jwt_extractor::JwtAuth;
use super::password;
use super::providers::{create_provider, repository as provider_repo};
use super::refresh_tokens;
use super::types::{
    AuthResponse, LoginRequest, MeResponse, OAuthAuthorizeQuery, OAuthCallbackQuery,
    RefreshTokenRequest, RegisterRequest,
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
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_USERNAME", "Username cannot be empty"),
        ));
    }
    if req.email.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_EMAIL", "Email cannot be empty"),
        ));
    }
    if let Err(msg) = password::validate_password_strength(&req.password) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_PASSWORD", msg),
        ));
    }

    // Check if username or email already exists. Closes 01-auth F-13
    // (Medium): the previous "Username" vs "Email" differential let an
    // attacker probe which of two values is already registered (user
    // enumeration). We now collapse both branches into the same
    // generic "ACCOUNT_EXISTS" response, leaking nothing about which
    // field collided. Server-side logs still record which one for
    // operator debugging.
    let username_taken = Repos
        .user
        .get_by_username(&req.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .is_some();
    let email_taken = Repos
        .user
        .get_by_email(&req.email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .is_some();
    if username_taken || email_taken {
        if username_taken {
            tracing::info!("Register conflict on username (logged for ops; client sees generic)");
        }
        if email_taken {
            tracing::info!("Register conflict on email (logged for ops; client sees generic)");
        }
        return Err((
            StatusCode::CONFLICT,
            AppError::new(
                StatusCode::CONFLICT,
                "ACCOUNT_EXISTS",
                "An account with these details already exists",
            ),
        ));
    }

    // Hash password
    let password_hash = password::hash_password(&req.password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Failed to hash password: {}", e)),
        )
    })?;

    // Create user
    let user = Repos
        .user
        .create(
            &req.username,
            &req.email,
            Some(password_hash),
            req.display_name,
            None,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Auto-assign user to default group
    Repos
        .auth
        .assign_user_to_default_group(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Emit UserCreated event asynchronously
    event_bus.emit_async(UserEvent::created(user.clone()));

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::CREATED, Json(AuthResponse { user, tokens })))
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
    // Check if external provider is specified
    if let Some(provider_name) = &req.provider {
        if provider_name != "local" {
            // External authentication (LDAP/OAuth)
            return login_with_provider(
                Repos.pool().clone(),
                jwt_service,
                &req.username,
                &req.password,
                provider_name,
            )
            .await;
        }
    }

    // Local password authentication.
    //
    // Closes 01-auth F-06 (Medium): the previous flow leaked
    // existence three ways: (a) returning early with no bcrypt when
    // the user didn't exist (~10ms timing differential), (b) a
    // distinct ACCOUNT_DISABLED error for valid-but-disabled accounts,
    // and (c) a distinct NO_PASSWORD error for OAuth-only accounts.
    // Combined, an attacker could enumerate registered emails.
    //
    // Defense:
    //   - Always run bcrypt verify against a precomputed dummy hash
    //     when the user / password is absent, so the timing matches a
    //     real verification call.
    //   - Collapse every failure into the same INVALID_CREDENTIALS
    //     response shape.
    //   - Log the real reason server-side for operator debugging.
    //
    // The dummy hash is precomputed once at first use; the password
    // input to bcrypt::verify is the user-supplied password (so the
    // hashing cost matches the input).
    static DUMMY_PWHASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let dummy_hash = DUMMY_PWHASH.get_or_init(|| {
        // Cost matches the application default (bcrypt::DEFAULT_COST).
        // A random unguessable value so even a length-equal guess
        // can't match.
        bcrypt::hash(uuid::Uuid::new_v4().to_string(), bcrypt::DEFAULT_COST)
            .expect("bcrypt dummy hash")
    });

    let user_opt = Repos
        .user
        .get_by_username_or_email(&req.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Pick the hash to verify: real user hash, dummy when missing or
    // no-password. Both code paths run bcrypt to keep timing flat.
    let (hash_to_verify, real_user_active, password_was_present) = match &user_opt {
        Some(u) => match u.password_hash.as_deref() {
            Some(h) => (h.to_string(), u.is_active, true),
            None => (dummy_hash.clone(), u.is_active, false),
        },
        None => (dummy_hash.clone(), false, false),
    };

    let verify_result =
        password::verify_password(&req.password, &hash_to_verify).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Password verification error: {}", e)),
            )
        })?;

    if !verify_result || !real_user_active || !password_was_present {
        if user_opt.is_none() {
            tracing::info!("Login failed: user not found");
        } else if !real_user_active {
            tracing::info!("Login failed: account disabled");
        } else if !password_was_present {
            tracing::info!("Login failed: no password (OAuth-only account)");
        } else {
            tracing::info!("Login failed: bad password");
        }
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("INVALID_CREDENTIALS", "Invalid username or password"),
        ));
    }

    let user = user_opt.expect("user_opt is Some past timing-equalised checks");

    // Update last login
    Repos
        .user
        .update_last_login(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(AuthResponse { user, tokens })))
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
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                AppError::not_found("Authentication provider"),
            )
        })?;

    // Create provider instance
    let provider = create_provider(&provider_config, pool.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e)),
        )
    })?;

    // Authenticate with external provider
    let auth_result = provider
        .authenticate(username, password)
        .await
        .map_err(|_e| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized(
                    "INVALID_CREDENTIALS",
                    format!("Invalid username or password"),
                ),
            )
        })?;

    // Try to find user via auth link
    let user_id = Repos
        .auth
        .find_user_by_auth_link(provider_config.id, &auth_result.external_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let user = if let Some(user_id) = user_id {
        // User exists, get it
        Repos
            .user
            .get_by_id(user_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?
    } else {
        // User doesn't exist - create new user with auth link and default group assignment
        let display_name = auth_result
            .attributes
            .display_name
            .unwrap_or_else(|| username.to_string());
        let email = auth_result.attributes.email;

        let new_user_id = Repos
            .auth
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
        Repos
            .user
            .get_by_id(new_user_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error("Failed to fetch newly created user"),
                )
            })?
    };

    // Check if user is active
    if !user.is_active {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled"),
        ));
    }

    // Update last login
    Repos
        .user
        .update_last_login(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(AuthResponse { user, tokens })))
}

/// POST /api/auth/refresh
/// Refresh access token using refresh token
#[debug_handler]
pub async fn refresh(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<RefreshTokenRequest>,
) -> ApiResult<Json<TokenPair>> {
    // Validate refresh token (signature + exp + iss + aud)
    let claims = jwt_service
        .validate_refresh_token(&req.refresh_token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Invalid user ID in token: {}", e)),
        )
    })?;

    // SECURITY: check the refresh token's jti against the whitelist
    // (refresh_tokens table). Closes 01-auth F-03 (refresh didn't rotate
    // the presented token — the old one kept minting access tokens for
    // up to 30 days).
    //
    // Tokens minted BEFORE this commit don't carry a jti claim; we let
    // those through unconditionally so existing sessions don't break on
    // the upgrade. Within ~30 days every active session is naturally
    // re-issued through the new code path and gets a jti, after which
    // unchecked legacy tokens can no longer exist.
    if let Some(jti_str) = claims.jti.as_deref() {
        let jti = uuid::Uuid::parse_str(jti_str).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("INVALID_TOKEN", "Invalid refresh token jti"),
            )
        })?;
        let active = refresh_tokens::is_active(Repos.pool(), jti)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        if !active {
            return Err((
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized(
                    "REFRESH_TOKEN_REVOKED",
                    "Refresh token has been revoked or already used",
                ),
            ));
        }
        // Revoke the presented refresh token NOW so it can't be used a
        // second time even if the new pair fails to land at the client.
        refresh_tokens::revoke(Repos.pool(), jti)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    // Get user from database
    let user = Repos
        .user
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("USER_NOT_FOUND", "User not found"),
            )
        })?;

    // Check if user is still active
    if !user.is_active {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled"),
        ));
    }

    // Generate new tokens with jti and register the new refresh token
    // in the whitelist before returning it.
    let token_pair_with_jti = jwt_service
        .generate_tokens_with_jti(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    refresh_tokens::register(
        Repos.pool(),
        token_pair_with_jti.refresh_jti,
        user.id,
        token_pair_with_jti.refresh_expires_at,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(token_pair_with_jti.pair)))
}

/// Documentation for refresh endpoint
pub fn refresh_docs(op: TransformOperation) -> TransformOperation {
    op.description("Refresh access token using refresh token")
        .id("Auth.refresh")
        .tag("auth")
        .response::<200, Json<TokenPair>>()
}

/// POST /api/auth/logout
/// Logout current user. Revokes all of the user's active refresh tokens
/// so subsequent calls to /auth/refresh fail with REFRESH_TOKEN_REVOKED.
/// Closes 01-auth F-02 (logout was a no-op).
///
/// The access token itself remains valid for the remainder of its TTL
/// (typically 24h). Clients must drop it from storage on logout. Server-
/// side access-token revocation would require either short TTLs (already
/// the design intent) or a per-request revocation check (deferred — adds
/// a DB hit to every authenticated request).
#[debug_handler]
pub async fn logout(auth: JwtAuth) -> ApiResult<()> {
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Invalid user ID in token: {}", e)),
        )
    })?;
    refresh_tokens::revoke_all_for_user(Repos.pool(), user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
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
pub async fn me(auth: JwtAuth) -> ApiResult<Json<MeResponse>> {
    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Invalid user ID in token: {}", e)),
        )
    })?;

    // Get user from database
    let user = Repos
        .user
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

    // Get effective permissions (union of user permissions + group permissions)
    let user_service = UserService::new(
        (**Repos.user).clone(),
        (**Repos.group).clone(),
    );
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
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                AppError::not_found("Authentication provider"),
            )
        })?;

    // Create provider instance
    let provider = create_provider(&provider_config, Repos.pool().clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e)),
        )
    })?;

    // SECURITY: ignore the user-supplied redirect_uri query parameter
    // and always use the server's canonical OAuth callback path. The
    // original implementation let `?redirect_uri=https://evil.com/` flow
    // through to the OAuth authorize call; well-configured providers
    // would reject the mismatch against their registered URI, but
    // misconfigured ones (which are common with self-hosted IdP setups)
    // would happily redirect the victim's browser to evil.com WITH the
    // OAuth `code` in the query string — evil.com can then exchange
    // the code for the access + ID token. Closes 01-auth F-07 (High).
    let redirect_uri = format!("/api/auth/oauth/{}/callback", provider_name);

    // Initialize OAuth flow
    let oauth_result = provider.init_oauth_flow(&redirect_uri).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("OAuth initialization failed: {}", e)),
        )
    })?;

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
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                AppError::not_found("Authentication provider"),
            )
        })?;

    // Create provider instance
    let provider = create_provider(&provider_config, Repos.pool().clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e)),
        )
    })?;

    // Handle OAuth callback
    let auth_result = provider
        .handle_oauth_callback(&query.code, &query.state, &query.state)
        .await
        .map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized(
                    "OAUTH_FAILED",
                    format!("OAuth authentication failed: {}", e),
                ),
            )
        })?;

    // Try to find user via auth link
    let user_id = Repos
        .auth
        .find_user_by_auth_link(provider_config.id, &auth_result.external_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(link_user_id) = user_id {
        let user = Repos
            .user
            .get_by_id(link_user_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

        // Check if user is active
        if !user.is_active {
            return Err((
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled"),
            ));
        }

        // Update last login
        Repos
            .user
            .update_last_login(user.id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        // Generate JWT tokens
        let tokens = jwt_service
            .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        // SECURITY: return the token in the URL FRAGMENT (#) rather than
        // the query (?). The fragment is not transmitted to the server,
        // not written to server access logs, not sent as the Referer on
        // subsequent navigations, and not indexed by search engines that
        // crawl the redirect chain. The frontend reads
        // window.location.hash on landing and immediately calls
        // history.replaceState to scrub it from browser history.
        //
        // Closes 01-auth F-01 (Critical): the previous '/?token=...'
        // form wrote the bearer token to browser history, Referer
        // headers, and every reverse-proxy access log on the path —
        // full account takeover blast radius from a single Referer leak
        // or shared browser session.
        Ok(Redirect::temporary(&format!(
            "/#token={}",
            tokens.access_token
        )))
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
