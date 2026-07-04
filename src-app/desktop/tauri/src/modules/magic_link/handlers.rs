//! Magic-link HTTP handlers.
//!
//! `issue` is admin-only (gated by `RequirePermissions<(RemoteAccessManage,)>`)
//! AND localhost-gated via the same Host-header middleware as the
//! rest of remote_access — a phone with a stolen admin token still
//! can't mint new magic links from outside the desktop.
//!
//! `exchange` is intentionally unauthenticated (that's the point of
//! the magic link). It's heavily rate-limited per peer IP — the
//! global tower_governor at 50/sec is the first line, and any
//! adversarial IP gets aggressive 401s with no information leak.

use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, http::StatusCode};
use chrono::{Duration, Utc};
use std::sync::Arc;

use ziee::{ApiResult, AppError};
use ziee::Repos;
use ziee::JwtService;
use ziee::AuthResponse;
use ziee::permissions::{RequirePermissions, with_permission};
use crate::modules::remote_access::permissions::RemoteAccessManage;

use super::models::{MagicLinkExchangeRequest, MagicLinkIssueResponse};
use super::repository::{MagicLinkRepository, hash_token};

/// Token TTL — long enough for the user to walk from the desktop
/// (showing the QR) to their phone (scanning it); short enough to
/// minimize the window where a captured QR screenshot is useful.
const TOKEN_TTL_SECS: i64 = 300; // 5 minutes

// =====================================================
// POST /api/auth/magic-link/issue
// =====================================================

#[debug_handler]
pub async fn issue(
    _: RequirePermissions<(RemoteAccessManage,)>,
) -> ApiResult<Json<MagicLinkIssueResponse>> {
    // Look up the admin user — single-admin desktop deployment.
    let admin = Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("admin user")))?;

    // Random 32-byte token, base64url-encoded for URL-safety.
    // rand 0.9: OsRng has been moved; use the rand::rng() entropy
    // source via the `Rng` trait. The internal RNG is thread-local
    // and cryptographically secure (ChaCha-based, reseeded from the
    // OS as needed).
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    use base64::Engine;
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::seconds(TOKEN_TTL_SECS);

    let repo = MagicLinkRepository::new(Repos.pool().clone());

    // Cap unused/unexpired magic links per admin — defense against
    // a buggy or compromised desktop UI looping on `/issue` (which
    // would otherwise mint a fresh DB row + JWT-equivalent every
    // call, all valid for 5 min). 5 is plenty for "I scanned the
    // wrong QR, gimme a new one a few times".
    const OUTSTANDING_CAP: i64 = 5;
    let outstanding = repo
        .count_active_for_user(admin.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if outstanding >= OUTSTANDING_CAP {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "TOO_MANY_OUTSTANDING_MAGIC_LINKS",
                "Too many active magic-link tokens for this user. Wait for the existing ones to expire (5 min) or use one to sign in.",
            ),
        ));
    }

    repo.insert(&token_hash, admin.id, expires_at)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    tracing::info!(
        user_id = %admin.id,
        // Log a hash prefix instead of the plaintext token prefix
        // (which previously leaked 48 bits of the token in logs).
        token_hash_prefix = %&token_hash[..8.min(token_hash.len())],
        expires_at = %expires_at,
        "auth.magic_link: issued"
    );

    Ok((
        StatusCode::OK,
        Json(MagicLinkIssueResponse { token, expires_at }),
    ))
}

pub fn issue_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessManage,)>(op)
        .id("Auth.magicLinkIssue")
        .tag("auth")
        .summary("Mint a one-time magic-link token (admin-only, localhost-gated).")
        .description(
            "Returns the plaintext token ONCE. The token is single-use, has a 5-minute TTL, \
             and authenticates as the admin user when redeemed via \
             POST /api/auth/magic-link/exchange. Only reachable from localhost (i.e. the \
             desktop UI); rejected when called via the public tunnel.",
        )
        .response::<200, Json<MagicLinkIssueResponse>>()
}

// =====================================================
// POST /api/auth/magic-link/exchange
// =====================================================
//
// Unauthenticated by design — this is what phones hit to log in.

#[debug_handler]
pub async fn exchange(
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<MagicLinkExchangeRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Always run the hash + DB lookup even when the token is empty,
    // so failure latency doesn't leak "did the token exist".
    let token_hash = hash_token(&req.token);
    let repo = MagicLinkRepository::new(Repos.pool().clone());
    let row = repo
        .consume(&token_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let Some(row) = row else {
        tracing::info!(
            // Hash prefix, not plaintext prefix — see issue handler.
            token_hash_prefix = %&token_hash[..8.min(token_hash.len())],
            "auth.magic_link: exchange rejected (invalid/expired/used)"
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized(
                "MAGIC_LINK_INVALID",
                "This link has expired or already been used. Get a fresh link from the desktop app.",
            ),
        ));
    };

    let user = Repos
        .user
        .get_by_id(row.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, AppError::not_found("user")))?;

    if !user.is_active {
        return Err((
            StatusCode::UNAUTHORIZED,
            AppError::unauthorized("ACCOUNT_DISABLED", "Account is disabled"),
        ));
    }

    Repos
        .user
        .update_last_login(user.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // The shared mint path: admin-configured lifetimes + a whitelisted
    // (jti-registered) refresh token. Without the whitelist, /auth/logout
    // can't revoke the phone's session — the token would stay usable for
    // the full refresh-token TTL, defeating the logout button's whole
    // purpose.
    let with_jti = ziee::refresh_tokens::mint_session_tokens(
        &jwt_service,
        user.id,
        &user.username,
        &user.email,
        user.is_admin,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    tracing::info!(
        user_id = %user.id,
        "auth.magic_link: exchange succeeded"
    );

    Ok((StatusCode::OK, Json(AuthResponse { user, tokens: with_jti.pair })))
}

pub fn exchange_docs(op: TransformOperation) -> TransformOperation {
    op.id("Auth.magicLinkExchange")
        .tag("auth")
        .summary("Exchange a magic-link token for a JWT pair (unauth, single-use).")
        .response::<200, Json<AuthResponse>>()
        .response_with::<401, (), _>(|r| {
            r.description("Token is invalid, expired, or already used")
        })
}
