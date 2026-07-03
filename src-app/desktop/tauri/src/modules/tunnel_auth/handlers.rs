//! Tunnel-aware auth handlers.

use std::sync::Arc;

use axum::{Extension, Json, debug_handler, http::StatusCode};
use ziee::{ApiResult, AppError, AuthResponse, JwtAuth, JwtService, Repos, TransformOperation, password};

use crate::modules::remote_access::middleware::is_localhost_host;
use crate::modules::remote_access::repository::RemoteAccessRepository;

use super::models::{AuthConfigResponse, ChangePasswordRequest, PasswordOnlyLoginRequest};

// =====================================================
// GET /api/auth/config — drives the login page render
// =====================================================
//
// Reads the inbound request's `Host` header (treating "is this
// localhost?" as the deployment-mode signal) and the singleton
// `remote_access_settings` row to compute the public, unauthenticated
// shape that the SPA needs to decide what login UI to render.

#[debug_handler]
pub async fn get_auth_config(
    headers: axum::http::HeaderMap,
) -> ApiResult<Json<AuthConfigResponse>> {
    let on_localhost = is_localhost_host(&headers);

    // Localhost (Tauri webview / dev / multi-user web deployment):
    // password auth is always allowed, username field is always
    // shown. The remote-access setting only governs tunneled
    // requests — desktop owners shouldn't be locked out of their
    // own machine by toggling password auth off.
    if on_localhost {
        return Ok((
            StatusCode::OK,
            Json(AuthConfigResponse {
                password_auth_enabled: true,
                magic_link_enabled: false,
                hide_username: false,
            }),
        ));
    }

    // Tunneled request: read the remote_access settings row.
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let settings = repo
        .get_row()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((
        StatusCode::OK,
        Json(AuthConfigResponse {
            password_auth_enabled: settings.password_auth_enabled,
            magic_link_enabled: true,
            hide_username: true,
        }),
    ))
}

pub fn get_auth_config_docs(op: TransformOperation) -> TransformOperation {
    op.id("Auth.getConfig")
        .tag("auth")
        .summary("Public auth config that drives the login page (no auth required).")
        .description(
            "Returns flags describing what login UI to render for this request. \
             Tunneled requests (Host header is not localhost) get hide_username=true \
             and password_auth_enabled mirrors the admin's Remote Access toggle. \
             Localhost requests always get the full multi-user UI behavior.",
        )
        .response::<200, Json<AuthConfigResponse>>()
}

// =====================================================
// POST /api/auth/login-password-only
// =====================================================
//
// Single-admin tunnel path: client supplies only a password, server
// authenticates as the admin user. Rejected with 403 when
// `password_auth_enabled` is OFF, so a stale tunnel URL can never
// suddenly start accepting password logins after the admin disabled
// it.

#[debug_handler]
pub async fn login_password_only(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PasswordOnlyLoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Authorization gate: even on localhost, this endpoint exists
    // only for the password-only UI. We require the admin to have
    // explicitly enabled password authentication for tunnel use OR
    // for the request to be on localhost (the desktop's own webview
    // sometimes routes through this endpoint depending on auth-config
    // branching). Tunnel + disabled = hard 403.
    if !is_localhost_host(&headers) {
        let repo = RemoteAccessRepository::new(Repos.pool().clone());
        let settings = repo
            .get_row()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        if !settings.password_auth_enabled {
            return Err((
                StatusCode::FORBIDDEN,
                AppError::new(
                    StatusCode::FORBIDDEN,
                    "PASSWORD_LOGIN_DISABLED",
                    "Password login is disabled on this server. Use the magic link from the desktop app.",
                ),
            ));
        }
    }

    // Always run bcrypt against the admin's real hash. Same
    // timing-equalising approach as the regular `login` handler — we
    // use a dummy hash when the admin row is missing (shouldn't
    // happen in practice but defensive).
    static DUMMY_PWHASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let dummy_hash = DUMMY_PWHASH.get_or_init(|| {
        bcrypt::hash(uuid::Uuid::new_v4().to_string(), bcrypt::DEFAULT_COST)
            .expect("bcrypt dummy hash")
    });

    let admin_opt = Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let (hash_to_verify, real_user_active, password_was_present) = match &admin_opt {
        Some(u) => match u.password_hash.as_deref() {
            Some(h) => (h.to_string(), u.is_active, true),
            None => (dummy_hash.clone(), u.is_active, false),
        },
        None => (dummy_hash.clone(), false, false),
    };

    let verify_result = ziee::password::verify_password(&req.password, &hash_to_verify)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Password verification error: {}", e)),
            )
        })?;

    if !verify_result || !real_user_active || !password_was_present {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("INVALID_CREDENTIALS", "Invalid password"),
        ));
    }

    let user = admin_opt.expect("admin_opt is Some past timing-equalised checks");

    Repos
        .user
        .update_last_login(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // The shared mint path: admin-configured lifetimes + a whitelisted
    // (jti-registered) refresh token so /auth/logout can revoke it — a
    // non-revocable token surviving logout is bad for a phone session on
    // a shared device.
    let with_jti = ziee::refresh_tokens::mint_session_tokens(
        &jwt_service,
        user.id,
        &user.username,
        &user.email,
        user.is_admin,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tokens = with_jti.pair;

    Ok((StatusCode::OK, Json(AuthResponse { user, tokens })))
}

pub fn login_password_only_docs(op: TransformOperation) -> TransformOperation {
    op.id("Auth.loginPasswordOnly")
        .tag("auth")
        .summary("Password-only login for the single-admin desktop deployment.")
        .description(
            "Authenticates as the admin user using only a password. Rejected with 403 when \
             called from a tunneled request and the admin has not enabled password \
             authentication via the Remote Access settings.",
        )
        .response::<200, Json<AuthResponse>>()
        .response_with::<401, (), _>(|r| r.description("Invalid password"))
        .response_with::<403, (), _>(|r| {
            r.description("Password login is disabled for tunneled requests")
        })
}

// =====================================================
// POST /api/users/me/password — change own password
// =====================================================
//
// Lives in the desktop crate because its sole consumer is the Remote
// Access feature — only the desktop installs `users.password_changed_at`
// (set by `Repos.user.update_password`) and gates password-auth
// toggling on it. A multi-user web server has no need for this handler;
// users change their own password via the normal account-settings flow.

#[debug_handler]
pub async fn change_password(
    auth: JwtAuth,
    Json(req): Json<ChangePasswordRequest>,
) -> ApiResult<()> {
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Invalid user ID in token: {}", e)),
        )
    })?;

    let user = Repos
        .user
        .get_by_id(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("user")))?;

    let current_hash = user.password_hash.as_deref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "NO_LOCAL_PASSWORD",
                "This account does not have a local password (signed up via OAuth?).",
            ),
        )
    })?;

    let ok = password::verify_password(&req.current_password, current_hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Password verification error: {}", e)),
        )
    })?;

    if !ok {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("INVALID_CREDENTIALS", "Current password is incorrect"),
        ));
    }

    if let Err(msg) = password::validate_password_strength(&req.new_password) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("WEAK_PASSWORD", msg),
        ));
    }

    let new_hash = password::hash_password(&req.new_password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Failed to hash password: {}", e)),
        )
    })?;

    Repos
        .user
        .update_password(user.id, &new_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn change_password_docs(op: TransformOperation) -> TransformOperation {
    op.id("Users.changeOwnPassword")
        .tag("users")
        .summary("Change the authenticated user's password.")
        .description(
            "Requires the current password as proof. On success, bumps \
             `users.password_changed_at` (used by the Remote Access module to gate \
             enabling password authentication).",
        )
        .response::<204, ()>()
        .response_with::<401, (), _>(|r| r.description("Current password incorrect"))
        .response_with::<400, (), _>(|r| r.description("New password fails strength check"))
}
