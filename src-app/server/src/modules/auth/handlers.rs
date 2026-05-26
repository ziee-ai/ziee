// Auth handlers

use aide::transform::TransformOperation;
use axum::{
    Extension, Form, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::user::events::UserEvent;
use crate::modules::user::UserService;

use super::jwt::{JwtService, TokenPair};
use super::jwt_extractor::JwtAuth;
use super::password;
use super::permissions::{AuthProvidersManage, AuthProvidersRead};
use super::providers::{AuthResult, create_provider, repository as provider_repo};
use super::refresh_tokens;
use super::types::{
    AppleCallbackForm, AuthProviderResponse, AuthResponse, CreateAuthProviderRequest,
    DeleteProviderResponse, LinkAccountRequest, LoginRequest, MeResponse, OAuthAuthorizeQuery,
    OAuthCallbackQuery, PublicProvider, PublicProvidersResponse, RefreshTokenRequest,
    RegisterRequest, TestProviderResponse, UpdateAuthProviderRequest,
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
    if let Some(provider_name) = &req.provider
        && provider_name != "local" {
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
                    "Invalid username or password".to_string(),
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
    headers: axum::http::HeaderMap,
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
    // and always use the server's canonical OAuth callback URL. The
    // original implementation let `?redirect_uri=https://evil.com/` flow
    // through to the OAuth authorize call; well-configured providers
    // would reject the mismatch against their registered URI, but
    // misconfigured ones (which are common with self-hosted IdP setups)
    // would happily redirect the victim's browser to evil.com WITH the
    // OAuth `code` in the query string — evil.com can then exchange
    // the code for the access + ID token. Closes 01-auth F-07 (High).
    //
    // The OAuth2 spec requires an absolute URL — derive scheme + host
    // from the inbound request. Reverse-proxy operators should ensure
    // their proxy forwards X-Forwarded-Proto so https survives the
    // hop; otherwise we fall back to http (the dev / tests default).
    // The path portion is server-controlled (provider_name comes from
    // URL routing matched against a string we built ourselves, not
    // user-controlled here).
    // SECURITY: derive redirect_uri from PROXY-SET headers ONLY when
    // the operator explicitly opted into trusting them via
    // `server.trust_forwarded_headers: true` (default false). When
    // the server is exposed directly (no reverse proxy), an attacker
    // can send `X-Forwarded-Host: evil.com` straight to the backend
    // and a permissive IdP (Keycloak wildcard, Dex, Authentik) will
    // happily hand the OAuth `code` to evil.com — same F-07 attack
    // class as the dropped Referer-derivation, just shifted to a
    // different header. With the flag off, derive from HOST only
    // (set by the client but only routable to OUR origin's IP).
    //
    // Never trust Referer in either mode — Referer is unconditionally
    // attacker-controllable.
    //
    // Dev: the Vite proxy is configured to forward X-Forwarded-Host
    // explicitly (see src-app/ui/vite.config.ts), so dev mode runs
    // with trust_forwarded_headers=true by setting it in
    // config/dev.yaml.
    fn safe_scheme(s: &str) -> Option<&str> {
        match s {
            "http" | "https" => Some(s),
            _ => None,
        }
    }
    let trust_proxy = super::trust_forwarded_headers();
    let scheme = if trust_proxy {
        headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| safe_scheme(s))
            .unwrap_or("http")
            .to_string()
    } else {
        "http".to_string()
    };
    let host = {
        let from_proxy = if trust_proxy {
            headers
                .get("x-forwarded-host")
                .and_then(|v| v.to_str().ok())
        } else {
            None
        };
        from_proxy
            .or_else(|| {
                headers
                    .get(axum::http::header::HOST)
                    .and_then(|v| v.to_str().ok())
            })
            .filter(|h| !h.is_empty())
            .map(|s| s.to_string())
    };
    let origin = match host {
        Some(h) => format!("{}://{}", scheme, h),
        None => {
            if cfg!(debug_assertions) {
                "http://localhost".to_string()
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    AppError::bad_request(
                        "OAUTH_MISCONFIGURED",
                        "Server cannot derive redirect URL",
                    ),
                ));
            }
        }
    };
    let redirect_uri = format!(
        "{}/api/auth/oauth/{}/callback",
        origin, provider_name
    );

    // Validate + capture return_to. We never round-trip it through
    // the provider URL — it lives on `oauth_sessions.return_to`
    // (see G3 in the plan). Only same-origin paths are accepted;
    // anything else (absolute URLs, protocol-relative `//host/...`,
    // backslash tricks) is silently dropped so the callback falls
    // back to `/`.
    let validated_return_to = validate_return_to(query.return_to.as_deref());

    // Initialize OAuth flow
    let oauth_result = provider
        .init_oauth_flow(&redirect_uri, validated_return_to.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("OAuth initialization failed: {}", e)),
            )
        })?;

    // Redirect to provider's authorization URL
    Ok(Redirect::temporary(&oauth_result.redirect_url))
}

/// Reject anything that isn't a same-origin path: must start with a
/// single `/` (not `//` — protocol-relative), no backslashes, no
/// control characters. Anything else returns None and the callback
/// falls back to `/`.
fn validate_return_to(rt: Option<&str>) -> Option<String> {
    let rt = rt?;
    if !rt.starts_with('/') || rt.starts_with("//") {
        return None;
    }
    if rt.bytes().any(|b| b == b'\\' || b < 0x20) {
        return None;
    }
    Some(rt.to_string())
}

/// GET /api/auth/oauth/{provider_name}/callback
/// Handle OAuth callback from provider (Google, Microsoft, generic
/// OIDC, etc. — anything that uses the `query` response mode).
#[debug_handler]
pub async fn oauth_callback(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Path(provider_name): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // SECURITY: same enumeration concern as oauth_callback_post —
    // looking up the provider by name first would expose distinct
    // 404 (unknown name) vs 401 (known name, bad state) statuses,
    // letting an attacker probe which provider names exist. Validate
    // the state first via oauth_sessions (which proves the request
    // was solicited AND tells us the provider via provider_id), and
    // collapse failure modes into a single neutral 400.
    let session = Repos
        .auth
        .get_oauth_session_by_state(&query.state)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let session = match session {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_STATE",
                    "Callback state is invalid or expired",
                ),
            ));
        }
    };
    let provider_config = provider_repo::get_provider_by_id(Repos.pool(), session.provider_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_STATE",
                    "Callback state is invalid or expired",
                ),
            )
        })?;
    if provider_config.name != provider_name {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_STATE",
                "Callback state is invalid or expired",
            ),
        ));
    }
    oauth_complete(jwt_service, provider_name, query.code, query.state, None).await
}

/// POST /api/auth/oauth/{provider_name}/callback
/// Apple Sign In's `response_mode=form_post` lands here. Same
/// decision tree as the GET path, plus first-time-only Apple `user`
/// JSON merging (Apple gives us the user's display name exactly
/// ONCE, in this body — persist it or lose it forever).
#[debug_handler]
pub async fn oauth_callback_post(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Path(provider_name): Path<String>,
    Form(form): Form<AppleCallbackForm>,
) -> Result<impl IntoResponse, (StatusCode, AppError)> {
    // SECURITY: only Apple uses response_mode=form_post. Reject POSTs
    // targeted at any other provider — without this gate, a hostile
    // cross-origin form could submit `(code, state)` against
    // /oauth/google/callback (cookie-less endpoint with no other
    // protection) and trigger an account-binding flow as if Google
    // had posted form_post.
    //
    // SECURITY: also single-flatten the error responses. Looking up
    // the provider by NAME then returning distinct 404/405/307s
    // would let an attacker enumerate which provider names exist
    // and which are Apple (migration 47 pre-seeds google/microsoft/
    // apple rows even when disabled). Instead: validate the state
    // first via the oauth_sessions row (which proves the request
    // was solicited and tells us the provider via provider_id), and
    // collapse all failure modes into a single neutral 400.
    let session = Repos
        .auth
        .get_oauth_session_by_state(&form.state)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let session = match session {
        Some(s) => s,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_STATE",
                    "Callback state is invalid or expired",
                ),
            ));
        }
    };
    let provider_config = provider_repo::get_provider_by_id(Repos.pool(), session.provider_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_STATE",
                    "Callback state is invalid or expired",
                ),
            )
        })?;
    if provider_config.provider_type != "apple"
        || provider_config.name != provider_name
    {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_STATE",
                "Callback state is invalid or expired",
            ),
        ));
    }
    oauth_complete(jwt_service, provider_name, form.code, form.state, form.user).await
}

/// Shared callback completion logic. The user has bounced back from
/// the OAuth provider; figure out which of the four landing states
/// they're in (returning user / first-broker-link required / new
/// user / nothing to do) and route accordingly.
async fn oauth_complete(
    jwt_service: Arc<JwtService>,
    provider_name: String,
    code: String,
    state: String,
    apple_user_json: Option<String>,
) -> Result<Redirect, (StatusCode, AppError)> {
    // Run the inner logic, then ALWAYS try to delete the oauth_sessions
    // row keyed by `state` — providers delete on success, but every
    // error path used to leave an orphan row that would only be reaped
    // by the cleanup job (or worse, never if the cleanup job isn't
    // running). Use of `let _ = ...` is deliberate: a delete failure
    // here is non-fatal (the row will be reaped by TTL), and we don't
    // want to mask the original error.
    let result =
        oauth_complete_inner(jwt_service, provider_name, code, &state, apple_user_json)
            .await;
    if result.is_err() {
        let _ = Repos.auth.delete_oauth_session(&state).await;
    }
    result
}

async fn oauth_complete_inner(
    jwt_service: Arc<JwtService>,
    provider_name: String,
    code: String,
    state: &str,
    apple_user_json: Option<String>,
) -> Result<Redirect, (StatusCode, AppError)> {
    let provider_config = provider_repo::get_provider_by_name(Repos.pool(), &provider_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            AppError::not_found("Authentication provider"),
        ))?;

    let provider = create_provider(&provider_config, Repos.pool().clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("PROVIDER_ERROR", format!("Provider error: {}", e)),
        )
    })?;

    // PEEK at the oauth_sessions row before handing off to the
    // provider — the provider deletes it on success and we need the
    // return_to for the final redirect. Errors here are non-fatal:
    // worst case we fall back to "/".
    let return_to = Repos
        .auth
        .get_oauth_session_by_state(state)
        .await
        .ok()
        .flatten()
        .and_then(|s| s.return_to);

    let mut auth_result = provider
        .handle_oauth_callback(&code, state, state)
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

    // Apple form_post: merge the first-time-only `user` JSON before
    // any DB writes so the new user gets the display name on row
    // creation. No-op for non-Apple providers.
    if let Some(user_json_str) = apple_user_json.as_deref() {
        merge_apple_user_json(&mut auth_result, user_json_str);
    }

    // SECURITY: drop the email if the provider didn't assert it as
    // verified. Without this, a sloppy IdP (or a provider that
    // simply omits the `email_verified` claim) can hand us an
    // unverified email, and our email-collision branch would later
    // bind a social identity to a victim's local account. Stripping
    // the email forces auto-provisioning with email=None — user has
    // to enter a real email out-of-band.
    if !email_verified_from_auth_result(&auth_result)
        && auth_result
            .external_email
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    {
        auth_result.external_email = None;
        auth_result.attributes.email = String::new();
    }

    let provider_id = provider_config.id;

    // ── 1. Existing link → returning user, just issue JWT ────────
    // Single UPDATE+RETURNING (`touch_auth_link_and_get_user_id`)
    // bumps last_login_at and returns the user_id in one round-trip.
    if let Some(user_id) = Repos
        .auth
        .touch_auth_link_and_get_user_id(provider_id, &auth_result.external_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        let user = Repos
            .user
            .get_by_id(user_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

        if !user.is_active {
            return Err((
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("ACCOUNT_DISABLED", "User account is disabled"),
            ));
        }

        Repos
            .user
            .update_last_login(user.id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        let tokens = jwt_service
            .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        return Ok(success_redirect(&tokens.access_token, return_to.as_deref()));
    }

    // ── 2. Email collision with an existing local account ───────
    //     → First-Broker-Link: do NOT auto-link, require password.
    if email_verified_from_auth_result(&auth_result) {
        if let Some(email) = auth_result.external_email.as_deref() {
            if !email.is_empty() {
                if let Some(target_user_id) = Repos
                    .auth
                    .find_user_by_email_for_linking(email)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
                {
                    let target_user = Repos
                        .user
                        .get_by_id(target_user_id)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
                        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

                    if target_user.password_hash.is_some() {
                        // Local-password account → standard FBL flow.
                        let link_token = Repos
                            .auth
                            .create_pending_link(
                                provider_id,
                                target_user_id,
                                &auth_result.external_id,
                                auth_result.external_email.as_deref(),
                                Some(&auth_result.metadata),
                            )
                            .await
                            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
                        return Ok(Redirect::temporary(&format!(
                            "/auth/link-account?link_token={}",
                            url::form_urlencoded::byte_serialize(link_token.as_bytes())
                                .collect::<String>()
                        )));
                    } else {
                        // External-only account with the same email exists.
                        // Refuse with a clear error — auto-linking these
                        // would let the user hijack the account.
                        return Err((
                            StatusCode::CONFLICT,
                            AppError::new(
                                StatusCode::CONFLICT,
                                "EMAIL_TAKEN_BY_EXTERNAL_ACCOUNT",
                                "An account with this email already exists via another login method. Sign in with that method instead.",
                            ),
                        ));
                    }
                }
            }
        }
    }

    // ── 3. No link, no collision → auto-provision a new user ────
    let username = ensure_unique_username(&auth_result.attributes.username).await?;
    let email = auth_result
        .external_email
        .clone()
        .filter(|e| !e.is_empty());
    let display_name = auth_result
        .attributes
        .display_name
        .clone()
        .unwrap_or_else(|| username.clone());

    if email.is_none() && username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "OAUTH_NO_IDENTITY",
                "Provider returned no email or username; cannot create an account.",
            ),
        ));
    }

    // Atomic provision: user row + auth_link + default-group
    // assignment in a single transaction. Partial failure (e.g.
    // unique-collision race on the auth_link) used to leave a
    // password-less orphan that locked the user out forever —
    // re-login would trip the email-collision branch and refuse.
    let new_user_id = Repos
        .auth
        .provision_external_user_atomic(
            &username,
            email.as_deref(),
            &display_name,
            provider_id,
            &auth_result.external_id,
            Some(&auth_result.metadata),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let user = Repos
        .user
        .get_by_id(new_user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error("Failed to fetch newly created user"),
            )
        })?;

    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(success_redirect(&tokens.access_token, return_to.as_deref()))
}

/// Was the email asserted as verified by the provider? Both shapes
/// happen in the wild: standard OIDC providers put it under
/// `metadata.user_info.email_verified` (boolean), Apple puts it
/// under `metadata.email_verified` (boolean — we coerced from
/// Apple's quirky string earlier).
fn email_verified_from_auth_result(r: &AuthResult) -> bool {
    let read = |v: &serde_json::Value| -> Option<bool> {
        v.as_bool()
            .or_else(|| v.as_str().map(|s| s.eq_ignore_ascii_case("true")))
    };
    if let Some(v) = r
        .metadata
        .get("user_info")
        .and_then(|ui| ui.get("email_verified"))
    {
        if let Some(b) = read(v) {
            return b;
        }
    }
    if let Some(v) = r.metadata.get("email_verified") {
        if let Some(b) = read(v) {
            return b;
        }
    }
    false
}

/// Append `2`, `3`, … to the username until we find one that's not
/// taken. Up to 999 attempts before giving up — a hard cap rather
/// than an infinite loop to avoid pathological cases.
async fn ensure_unique_username(
    base: &str,
) -> Result<String, (StatusCode, AppError)> {
    let mut candidate = base.trim().to_string();
    if candidate.is_empty() {
        candidate = "user".to_string();
    }
    if Repos
        .user
        .get_by_username(&candidate)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .is_none()
    {
        return Ok(candidate);
    }
    for n in 2..=999u32 {
        let next = format!("{}{}", candidate, n);
        if Repos
            .user
            .get_by_username(&next)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .is_none()
        {
            return Ok(next);
        }
    }
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        AppError::internal_error("Could not derive a unique username"),
    ))
}

/// Merge Apple's first-auth-only `user` form field into the
/// AuthResult. The id_token has `sub` and `email` but never `name`;
/// `name` arrives in this body exactly once and only on first auth.
fn merge_apple_user_json(auth_result: &mut AuthResult, user_json_str: &str) {
    #[derive(serde::Deserialize)]
    struct AppleUser {
        name: Option<AppleName>,
        email: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct AppleName {
        #[serde(rename = "firstName")]
        first_name: Option<String>,
        #[serde(rename = "lastName")]
        last_name: Option<String>,
    }
    let Ok(parsed) = serde_json::from_str::<AppleUser>(user_json_str) else {
        return;
    };
    // SECURITY: the `user` payload comes from Apple's POST body, NOT
    // the signed id_token. Only trust the email if the id_token didn't
    // contain one AND the form value looks like a private-relay
    // address. Otherwise an attacker who can capture a (code, state)
    // pair could supply an arbitrary `email` here and trigger FBL
    // against a victim's account.
    if let Some(email) = parsed.email {
        let id_token_had_email = auth_result
            .external_email
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let looks_like_relay =
            email.to_ascii_lowercase().ends_with("@privaterelay.appleid.com");
        if !id_token_had_email && looks_like_relay {
            auth_result.external_email = Some(email.clone());
            auth_result.attributes.email = email;
        }
    }
    if let Some(name) = parsed.name {
        let first = name.first_name.clone();
        let last = name.last_name.clone();
        if auth_result.attributes.first_name.is_none() {
            auth_result.attributes.first_name = first.clone();
        }
        if auth_result.attributes.last_name.is_none() {
            auth_result.attributes.last_name = last.clone();
        }
        if auth_result.attributes.display_name.is_none() {
            auth_result.attributes.display_name = match (first, last) {
                (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
                (Some(f), None) => Some(f),
                (None, Some(l)) => Some(l),
                _ => None,
            };
        }
    }
}

/// Build the post-auth redirect. The access token rides in the URL
/// **fragment** (`#token=…`) so it does not appear in server access
/// logs, Referer headers, or browser history. The SPA's
/// `/auth/callback` page reads the fragment then immediately calls
/// `history.replaceState` to scrub it.
fn success_redirect(access_token: &str, return_to: Option<&str>) -> Redirect {
    let target = return_to.unwrap_or("/");
    let fragment = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", access_token)
        .append_pair("return_to", target)
        .finish();
    Redirect::temporary(&format!("/auth/callback#{}", fragment))
}

/// POST /api/auth/link-account
/// First-Broker-Login confirmation. The user proves ownership of an
/// existing local account by entering its password; on success we
/// atomically create the user_auth_links row + issue a JWT. The
/// pending row is consumed (deleted) regardless of outcome on success.
#[debug_handler]
pub async fn link_account(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<LinkAccountRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Peek (don't consume) so a wrong-password attempt doesn't burn
    // the single-use token — the user gets to retry without
    // re-running the entire OAuth dance. The token is still
    // single-use: we delete it on the FIRST successful password +
    // link insertion.
    let pending = Repos
        .auth
        .peek_pending_link(&req.link_token)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized(
                    "INVALID_LINK_TOKEN",
                    "Link token is invalid, already used, or expired",
                ),
            )
        })?;

    // Authorization checks BEFORE bumping the attempts counter so
    // an OAuth-only target account (no password_hash) doesn't burn
    // 5 attempts before the user sees a useful error.
    let user = Repos
        .user
        .get_by_id(pending.target_user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("User")))?;

    if !user.is_active {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("ACCOUNT_DISABLED", "Account is disabled"),
        ));
    }

    let pw_hash = user.password_hash.as_deref().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("INVALID_CREDENTIALS", "Invalid credentials"),
        )
    })?;

    // SECURITY: per-token brute-force gate. The global rate limiter
    // caps requests per IP; this caps requests per token, defeating
    // distributed brute-force. Hard ceiling 5 attempts — past that
    // the token is invalidated. Runs after the is_active +
    // password_hash checks so legitimate misuse-detection errors
    // don't burn attempts.
    const LINK_TOKEN_MAX_ATTEMPTS: i32 = 5;
    let attempt_n = Repos
        .auth
        .bump_pending_link_attempts(&req.link_token)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        // None means the row was concurrently consumed or just
        // expired between our peek and our bump. Surface as the
        // honest "this token is no longer valid" rather than
        // masquerading as a brute-force throttle.
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized(
                    "INVALID_LINK_TOKEN",
                    "Link token is invalid, already used, or expired",
                ),
            )
        })?;
    if attempt_n > LINK_TOKEN_MAX_ATTEMPTS {
        let _ = Repos.auth.delete_pending_link(&req.link_token).await;
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "TOO_MANY_ATTEMPTS",
                "Too many failed attempts. Restart the sign-in flow.",
            ),
        ));
    }

    let ok = password::verify_password(&req.password, pw_hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Password verification failed: {}", e)),
        )
    })?;
    if !ok {
        // Pending row intentionally preserved for retry.
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("INVALID_CREDENTIALS", "Invalid credentials"),
        ));
    }

    Repos
        .auth
        .create_auth_link_with_data(
            user.id,
            pending.provider_id,
            &pending.external_id,
            pending.external_email.as_deref(),
            pending.external_data.as_ref(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Consume the pending token now that the link is bound.
    let _ = Repos.auth.delete_pending_link(&req.link_token).await;

    Repos
        .user
        .update_last_login(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(AuthResponse { user, tokens })))
}

pub fn link_account_docs(op: TransformOperation) -> TransformOperation {
    op.description(
        "Confirm a First-Broker-Login pending link by proving ownership of \
         the existing local account with its password. Returns a fresh JWT \
         pair on success.",
    )
    .id("Auth.linkAccount")
    .tag("auth")
    .response::<200, Json<AuthResponse>>()
}

/// GET /api/auth/providers — public list of enabled providers for
/// the login page. Returns ONLY the fields the login UI needs;
/// never exposes config / secrets / tenant IDs.
#[debug_handler]
pub async fn list_public_providers() -> ApiResult<Json<PublicProvidersResponse>> {
    let rows = provider_repo::list_public_providers(Repos.pool())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let providers: Vec<PublicProvider> = rows
        .into_iter()
        .map(|p| {
            let display_name = p
                .config
                .get("display_name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| default_display_name(&p.name, &p.provider_type));
            PublicProvider {
                name: p.name,
                provider_type: p.provider_type,
                display_name,
            }
        })
        .collect();
    Ok((StatusCode::OK, Json(PublicProvidersResponse { providers })))
}

pub fn list_public_providers_docs(op: TransformOperation) -> TransformOperation {
    op.description(
        "List enabled third-party auth providers for the login page. Public \
         endpoint; returns only display fields, never config or secrets.",
    )
    .id("Auth.listProviders")
    .tag("auth")
    .response::<200, Json<PublicProvidersResponse>>()
}

fn default_display_name(name: &str, provider_type: &str) -> String {
    match provider_type {
        "apple" => "Sign in with Apple".to_string(),
        _ => format!("Sign in with {}", name),
    }
}

// =====================================================
// Admin: Auth Provider CRUD
// =====================================================
//
// All handlers below are gated through the typed permission
// extractor — never hand-rolled. The list endpoint requires
// `auth_providers::read`; everything mutating + the test endpoint
// requires `auth_providers::manage`. Administrators-group members
// get both implicitly via the `*` wildcard, so no seed grants needed.

/// Sensitive keys whose values are masked in any GET / list response.
const SENSITIVE_CONFIG_KEYS: &[&str] = &["client_secret", "bind_password", "private_key_path"];

/// Mask sentinel used in admin-list GET responses. We treat the
/// literal sentinel as "empty" on writes too — otherwise an admin
/// who clicks Save in the EditDrawer without retyping the password
/// would persist the bullet string `"••••••"` as the new secret,
/// destroying the real one (HIGH bug found by 2026-05-25 audit).
const MASK_SENTINEL: &str = "••••••";

/// Mask sensitive values inside an auth_providers.config JSONB
/// payload. Returns a cloned + masked copy; the original (with real
/// secrets) stays in the DB.
fn mask_provider_config(config: &serde_json::Value) -> serde_json::Value {
    let mut masked = config.clone();
    if let serde_json::Value::Object(map) = &mut masked {
        for key in SENSITIVE_CONFIG_KEYS {
            if let Some(v) = map.get_mut(*key) {
                if v.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                    *v = serde_json::Value::String(MASK_SENTINEL.to_string());
                }
            }
        }
    }
    masked
}

fn provider_to_response(p: super::providers::models::AuthProvider) -> AuthProviderResponse {
    AuthProviderResponse {
        config: mask_provider_config(&p.config),
        id: p.id,
        name: p.name,
        provider_type: p.provider_type,
        enabled: p.enabled,
        created_at: p.created_at,
        updated_at: p.updated_at,
        last_test_at: p.last_test_at,
        last_test_ok: p.last_test_ok,
        last_test_message: p.last_test_message,
    }
}

/// GET /api/admin/auth-providers
#[debug_handler]
pub async fn admin_list_providers(
    _: RequirePermissions<(AuthProvidersRead,)>,
) -> ApiResult<Json<Vec<AuthProviderResponse>>> {
    let rows = provider_repo::list_providers(Repos.pool())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let resp: Vec<AuthProviderResponse> = rows.into_iter().map(provider_to_response).collect();
    Ok((StatusCode::OK, Json(resp)))
}

pub fn admin_list_providers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersRead,)>(op)
        .id("AuthProviders.list")
        .tag("auth-providers")
        .summary("List all configured auth providers (secrets masked)")
        .response::<200, Json<Vec<AuthProviderResponse>>>()
}

/// POST /api/admin/auth-providers
#[debug_handler]
pub async fn admin_create_provider(
    _: RequirePermissions<(AuthProvidersManage,)>,
    Json(req): Json<CreateAuthProviderRequest>,
) -> ApiResult<Json<AuthProviderResponse>> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_NAME", "Provider name cannot be empty"),
        ));
    }
    // "local" is the built-in password provider — not creatable via
    // this endpoint (creating a second one leaves the login routing
    // in undefined state).
    let allowed_types = ["oidc", "oauth2", "apple", "ldap"];
    if !allowed_types.contains(&req.provider_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_PROVIDER_TYPE",
                format!(
                    "provider_type must be one of: {}",
                    allowed_types.join(", ")
                ),
            ),
        ));
    }
    let row = provider_repo::create_provider(
        Repos.pool(),
        req.name.trim(),
        req.provider_type.as_str(),
        req.enabled,
        &req.config,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok((StatusCode::CREATED, Json(provider_to_response(row))))
}

pub fn admin_create_provider_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersManage,)>(op)
        .id("AuthProviders.create")
        .tag("auth-providers")
        .summary("Create a new auth provider")
        .response::<201, Json<AuthProviderResponse>>()
}

/// PUT /api/admin/auth-providers/{id}
/// Empty `client_secret` in the patch config preserves the existing
/// value — so admins can edit other fields without re-entering
/// secrets they don't have at hand.
#[debug_handler]
pub async fn admin_update_provider(
    _: RequirePermissions<(AuthProvidersManage,)>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateAuthProviderRequest>,
) -> ApiResult<Json<AuthProviderResponse>> {
    // If config is being patched, merge sensitive empty fields with
    // the existing row to preserve secrets.
    let final_config = if let Some(mut new_config) = req.config {
        let existing = provider_repo::get_provider_by_id(Repos.pool(), id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Auth provider")))?;
        preserve_sensitive_fields(&existing.config, &mut new_config);
        Some(new_config)
    } else {
        None
    };

    let row = provider_repo::update_provider(
        Repos.pool(),
        id,
        req.name.as_deref().map(str::trim),
        req.enabled,
        final_config.as_ref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok((StatusCode::OK, Json(provider_to_response(row))))
}

/// For each `SENSITIVE_CONFIG_KEYS`: if it's missing or empty in
/// `new_config`, copy the existing value over. Lets admins PATCH a
/// provider row without re-entering secrets.
fn preserve_sensitive_fields(
    existing: &serde_json::Value,
    new_config: &mut serde_json::Value,
) {
    let (existing_obj, new_obj) = match (existing, new_config) {
        (serde_json::Value::Object(e), serde_json::Value::Object(n)) => (e, n),
        _ => return,
    };
    for key in SENSITIVE_CONFIG_KEYS {
        // Treat absent, empty string, AND the masking sentinel as
        // "leave existing value alone." The sentinel branch matters
        // because the admin EditDrawer GETs a masked config + sends
        // it straight back on Save; without this guard the bullets
        // would replace the real secret.
        let new_should_preserve = new_obj
            .get(*key)
            .map(|v| {
                v.as_str()
                    .map(|s| s.is_empty() || s == MASK_SENTINEL)
                    .unwrap_or(false)
            })
            .unwrap_or(true);
        if new_should_preserve {
            if let Some(existing_val) = existing_obj.get(*key) {
                new_obj.insert((*key).to_string(), existing_val.clone());
            }
        }
    }
}

pub fn admin_update_provider_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersManage,)>(op)
        .id("AuthProviders.update")
        .tag("auth-providers")
        .summary("Update an auth provider (empty client_secret preserves existing)")
        .response::<200, Json<AuthProviderResponse>>()
}

/// DELETE /api/admin/auth-providers/{id}
#[debug_handler]
pub async fn admin_delete_provider(
    _: RequirePermissions<(AuthProvidersManage,)>,
    Path(id): Path<uuid::Uuid>,
) -> ApiResult<Json<DeleteProviderResponse>> {
    let affected = provider_repo::count_links_for_provider(Repos.pool(), id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let n = provider_repo::delete_provider(Repos.pool(), id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, AppError::not_found("Auth provider")));
    }
    Ok((
        StatusCode::OK,
        Json(DeleteProviderResponse {
            deleted: true,
            affected_user_links: affected,
        }),
    ))
}

pub fn admin_delete_provider_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersManage,)>(op)
        .id("AuthProviders.delete")
        .tag("auth-providers")
        .summary("Delete an auth provider (cascades user_auth_links)")
        .response::<200, Json<DeleteProviderResponse>>()
}

/// POST /api/admin/auth-providers/{id}/test
/// Run the provider's `test_connection`. Discovery + dummy
/// token-exchange probe for OIDC / Apple; URL-syntax check for
/// OAuth2. Returns 200 always; success / failure is in the body so
/// the admin UI can render a nicer inline message than a non-200.
/// Persists the result on the auth_providers row (`last_test_at`,
/// `last_test_ok`, `last_test_message`) so the result survives a
/// page reload.
#[debug_handler]
pub async fn admin_test_provider(
    _: RequirePermissions<(AuthProvidersManage,)>,
    Path(id): Path<uuid::Uuid>,
) -> ApiResult<Json<TestProviderResponse>> {
    let row = provider_repo::get_provider_by_id(Repos.pool(), id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("Auth provider")))?;

    let result = run_test_for_row(row).await;
    // Persist the outcome regardless of pass/fail so the row reflects
    // current state on the next list call.
    if let Err(e) = provider_repo::record_test_result(
        Repos.pool(),
        id,
        result.ok,
        &result.message,
    )
    .await
    {
        tracing::warn!(error = ?e, "failed to persist auth-provider test result");
    }
    Ok((StatusCode::OK, Json(result)))
}

pub fn admin_test_provider_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersManage,)>(op)
        .id("AuthProviders.test")
        .tag("auth-providers")
        .summary("Test the auth provider's connection (probes discovery + credentials)")
        .response::<200, Json<TestProviderResponse>>()
}

/// POST /api/admin/auth-providers/test-config
/// Test a provider config WITHOUT saving it to the database. Used by
/// the EditDrawer's "Test config" button so admins can verify their
/// inputs before committing. Body is the same shape as Create — we
/// just don't persist the row. Result is not stored anywhere (no row
/// to attach to).
#[debug_handler]
pub async fn admin_test_provider_config(
    _: RequirePermissions<(AuthProvidersManage,)>,
    Json(req): Json<CreateAuthProviderRequest>,
) -> ApiResult<Json<TestProviderResponse>> {
    // Same allowlist as admin_create_provider — refuses "local" + any
    // unknown type. Without this, an admin could probe arbitrary
    // LDAP/OIDC servers via the test endpoint (lower-impact than
    // SSRF since admin already has manage permission, but still
    // good hygiene).
    let allowed_types = ["oidc", "oauth2", "apple", "ldap"];
    if !allowed_types.contains(&req.provider_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_PROVIDER_TYPE",
                format!(
                    "provider_type must be one of: {}",
                    allowed_types.join(", ")
                ),
            ),
        ));
    }
    let transient = super::providers::models::AuthProvider {
        id: uuid::Uuid::nil(),
        name: req.name,
        provider_type: req.provider_type,
        enabled: true,
        config: req.config,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_test_at: None,
        last_test_ok: None,
        last_test_message: None,
    };
    let result = run_test_for_row(transient).await;
    Ok((StatusCode::OK, Json(result)))
}

pub fn admin_test_provider_config_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AuthProvidersManage,)>(op)
        .id("AuthProviders.testConfig")
        .tag("auth-providers")
        .summary("Test a provider config without saving (used by the EditDrawer)")
        .response::<200, Json<TestProviderResponse>>()
}

/// Shared core: build the provider in-memory, run test_connection,
/// massage the result into a TestProviderResponse. Used by both the
/// per-row /test endpoint and the pre-save /test-config endpoint.
async fn run_test_for_row(
    mut row: super::providers::models::AuthProvider,
) -> TestProviderResponse {
    // The provider factory refuses to construct a disabled provider
    // (so the normal login flow stops early). Force-enable a copy
    // here so admins can test config BEFORE flipping the switch
    // — the row in the DB is untouched.
    row.enabled = true;
    let provider = match create_provider(&row, Repos.pool().clone()) {
        Ok(p) => p,
        Err(e) => {
            return TestProviderResponse {
                ok: false,
                message: format!("Configuration error: {}", e),
            };
        }
    };
    match provider.test_connection().await {
        Ok(msg) => TestProviderResponse { ok: true, message: msg },
        Err(e) => TestProviderResponse {
            ok: false,
            message: format!("{}", e),
        },
    }
}
