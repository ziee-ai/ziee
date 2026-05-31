//! HTTP handlers for the remote_access module.
//!
//! All handlers are gated by `RequirePermissions<(RemoteAccessRead,)>`
//! or `<(RemoteAccessManage,)>`. The route group is ALSO wrapped by
//! the localhost-Host middleware in `middleware.rs` as defense in
//! depth — a phone with a stolen admin token can't disable the
//! tunnel from a tunneled request because the Host header gives the
//! request away.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};

use ziee::{ApiResult, AppError};
use ziee::Repos;
use ziee::permissions::{RequirePermissions, with_permission};

use super::models::{
    RemoteAccessSettingsResponse, RemoteAccessSettingsRow, RemoteAccessStatusResponse,
    SetAdminPasswordRequest, TunnelStartResponse, TunnelStateKind,
    UpdateRemoteAccessSettingsRequest,
};
use super::permissions::{RemoteAccessManage, RemoteAccessRead};
use super::repository::RemoteAccessRepository;
use super::state::{local_server_port, tunnel_driver};
use super::tunnel::tunnel_error_to_api;

// =====================================================
// GET /api/remote-access/status
// =====================================================

#[debug_handler]
pub async fn get_status(
    _: RequirePermissions<(RemoteAccessRead,)>,
) -> ApiResult<Json<RemoteAccessStatusResponse>> {
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let row = repo
        .get_row()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let password_rotated = admin_password_rotated().await?;

    let driver = tunnel_driver();
    let status = driver.0.status().await;

    Ok((
        StatusCode::OK,
        Json(RemoteAccessStatusResponse {
            password_rotated,
            password_auth_enabled: row.password_auth_enabled,
            auth_token_set: row.ngrok_auth_token_enc.is_some(),
            ngrok_domain: row.ngrok_domain,
            auto_start_tunnel: row.auto_start_tunnel,
            tunnel_state: status.state,
            public_url: status.public_url,
            last_error: status.last_error,
            started_at: status.started_at,
        }),
    ))
}

pub fn get_status_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessRead,)>(op)
        .id("RemoteAccess.getStatus")
        .tag("remote-access")
        .summary("Get the combined remote-access status (settings + live tunnel state).")
        .response::<200, Json<RemoteAccessStatusResponse>>()
}

// =====================================================
// GET /api/remote-access/settings
// =====================================================

#[debug_handler]
pub async fn get_settings(
    _: RequirePermissions<(RemoteAccessRead,)>,
) -> ApiResult<Json<RemoteAccessSettingsResponse>> {
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let row = repo
        .get_row()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok((StatusCode::OK, Json(row_to_response(row))))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessRead,)>(op)
        .id("RemoteAccess.getSettings")
        .tag("remote-access")
        .summary("Get the remote-access settings (never echoes the ngrok auth token).")
        .response::<200, Json<RemoteAccessSettingsResponse>>()
}

// =====================================================
// PUT /api/remote-access/settings
// =====================================================

#[debug_handler]
pub async fn update_settings(
    _: RequirePermissions<(RemoteAccessManage,)>,
    Json(req): Json<UpdateRemoteAccessSettingsRequest>,
) -> ApiResult<Json<RemoteAccessSettingsResponse>> {
    // Validate invariants up-front. The CHECK constraint on the table
    // covers auto_start+domain, but we want a precise 422 error instead
    // of a generic DB violation.
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let current = repo
        .get_row()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Compute "what the row would look like after the patch":
    let next_domain = match &req.ngrok_domain {
        Some(Some(d)) => Some(d.clone()),
        Some(None) => None,
        None => current.ngrok_domain.clone(),
    };
    let next_auto_start = req.auto_start_tunnel.unwrap_or(current.auto_start_tunnel);
    let next_password_auth = req
        .password_auth_enabled
        .unwrap_or(current.password_auth_enabled);

    // Invariant 1: auto-start requires a fixed domain. UX rule: if
    // the user clears the domain while auto-start was on, auto-flip
    // it off instead of erroring. Refuses only when they're
    // explicitly setting auto_start=true with no domain.
    let mut adjusted_auto_start = req.auto_start_tunnel;
    if next_auto_start && next_domain.is_none() {
        if matches!(req.auto_start_tunnel, Some(true)) && req.ngrok_domain.is_none() {
            // User said "turn auto-start on" without touching domain;
            // and there's no existing domain. That's a true 422.
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                AppError::bad_request(
                    "AUTO_START_REQUIRES_DOMAIN",
                    "Auto-start requires a fixed ngrok domain. Set a custom domain first.",
                ),
            ));
        }
        // Otherwise: domain was cleared in this PUT while auto_start
        // was on — silently flip auto_start to false.
        adjusted_auto_start = Some(false);
    }

    // Invariant 2: enabling password auth requires a rotated admin
    // password. Checked on every save where the post-state would have
    // `password_auth_enabled=true`, NOT just on the false→true edge —
    // if anything ever clears `password_changed_at` (DB restore from
    // an older snapshot, future force-rotate flow), a stale `true`
    // flag wouldn't survive the next admin save.
    if next_password_auth {
        let rotated = admin_password_rotated().await?;
        if !rotated {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                AppError::bad_request(
                    "PASSWORD_NOT_ROTATED",
                    "Set a strong admin password before enabling password authentication. The bootstrap default is a well-known string and would be a security risk over a public tunnel.",
                ),
            ));
        }
    }

    let updated = repo
        .update_settings(
            req.ngrok_auth_token,
            req.ngrok_domain,
            adjusted_auto_start,
            req.password_auth_enabled,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok((StatusCode::OK, Json(row_to_response(updated))))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessManage,)>(op)
        .id("RemoteAccess.updateSettings")
        .tag("remote-access")
        .summary("Update remote-access settings (ngrok token, domain, auto-start, password-auth).")
        .description(
            "Three-state semantics per field: missing key = preserve, null = clear, value = set. \
             Token plaintext is encrypted at rest and never echoed back.",
        )
        .response::<200, Json<RemoteAccessSettingsResponse>>()
}

// =====================================================
// POST /api/remote-access/tunnel/start
// =====================================================

#[debug_handler]
pub async fn start_tunnel(
    _: RequirePermissions<(RemoteAccessManage,)>,
) -> ApiResult<Json<TunnelStartResponse>> {
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let settings = repo
        .get_settings()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let token = settings.ngrok_auth_token.ok_or_else(|| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            AppError::bad_request(
                "TUNNEL_TOKEN_MISSING",
                "No ngrok auth token configured. Save it in the Remote Access settings first.",
            ),
        )
    })?;

    let driver = tunnel_driver();
    let url = driver
        .0
        .start(&token, settings.ngrok_domain.as_deref(), local_server_port())
        .await
        .map_err(tunnel_error_to_api)?;

    Ok((
        StatusCode::OK,
        Json(TunnelStartResponse {
            public_url: url,
            started_at: chrono::Utc::now(),
        }),
    ))
}

pub fn start_tunnel_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessManage,)>(op)
        .id("RemoteAccess.startTunnel")
        .tag("remote-access")
        .summary("Start the ngrok tunnel using the saved auth token + optional domain.")
        .response::<200, Json<TunnelStartResponse>>()
        .response_with::<409, (), _>(|r| r.description("Tunnel already running"))
        .response_with::<422, (), _>(|r| r.description("No auth token configured"))
}

// =====================================================
// POST /api/remote-access/tunnel/stop
// =====================================================

#[debug_handler]
pub async fn stop_tunnel(_: RequirePermissions<(RemoteAccessManage,)>) -> ApiResult<()> {
    let driver = tunnel_driver();
    driver.0.stop().await.map_err(tunnel_error_to_api)?;
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn stop_tunnel_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessManage,)>(op)
        .id("RemoteAccess.stopTunnel")
        .tag("remote-access")
        .summary("Stop the active ngrok tunnel. Idempotent.")
        .response::<204, ()>()
}

// =====================================================
// Helpers
// =====================================================

/// Look up the admin user and check whether their password has been
/// rotated since bootstrap. Returns false if the user is missing
/// (which would itself be a bug — desktop bootstrap creates the
/// admin user before any of these handlers can fire).
pub(crate) async fn admin_password_rotated() -> Result<bool, (StatusCode, AppError)> {
    let admin = Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(admin
        .and_then(|u| u.password_changed_at)
        .is_some())
}

fn row_to_response(row: RemoteAccessSettingsRow) -> RemoteAccessSettingsResponse {
    RemoteAccessSettingsResponse {
        auth_token_set: row.ngrok_auth_token_enc.is_some(),
        ngrok_domain: row.ngrok_domain,
        auto_start_tunnel: row.auto_start_tunnel,
        password_auth_enabled: row.password_auth_enabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

// Suppress unused-warning when TunnelStateKind isn't directly used.
#[allow(dead_code)]
fn _kind_marker(_: TunnelStateKind) {}

// =====================================================
// POST /api/remote-access/admin-password
// =====================================================
//
// Desktop-specific: set the admin password without requiring the
// current one. The localhost-Host middleware that wraps this route
// is the auth proof — only someone with shell-level access to the
// desktop machine (or the embedded Tauri webview itself) can call
// it. Requiring the well-known bootstrap default
// (`desktop-auto-login`) as proof is friction without security
// since that string is published in CLAUDE.md.

#[debug_handler]
pub async fn set_admin_password(
    auth: RequirePermissions<(RemoteAccessManage,)>,
    Json(req): Json<SetAdminPasswordRequest>,
) -> ApiResult<()> {
    use ziee::password;

    // Belt-and-suspenders: the localhost-Host middleware is the
    // primary auth proof, but the RequirePermissions extractor
    // resolves to whichever user holds `remote_access::manage`. On a
    // multi-user install (not the supported deployment model — but
    // possible if the admin manually creates groups), a sub-admin
    // with that permission could otherwise reset the root admin's
    // password from localhost. Refuse unless the acting user IS the
    // admin we're about to mutate.
    if auth.user.username != "admin" || !auth.user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            AppError::forbidden(
                "NOT_ROOT_ADMIN",
                "Only the root admin user can rotate the admin password via this endpoint.",
            ),
        ));
    }

    // Strength check (the same validator the multi-user endpoint
    // uses; centralizes the policy).
    if let Err(msg) = password::validate_password_strength(&req.new_password) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("WEAK_PASSWORD", msg),
        ));
    }

    // Look up admin. The bootstrap creates this row before any
    // handler can fire, so a missing row is a real bug, not a
    // client error.
    let admin = Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(
                    "admin user not found — this should never happen on a desktop install",
                ),
            )
        })?;

    let new_hash = password::hash_password(&req.new_password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Failed to hash password: {}", e)),
        )
    })?;

    Repos
        .user
        .update_password(admin.id, &new_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    tracing::info!(
        admin_id = %admin.id,
        "remote_access: admin password reset via localhost-gated endpoint"
    );

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn set_admin_password_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RemoteAccessManage,)>(op)
        .id("RemoteAccess.setAdminPassword")
        .tag("remote-access")
        .summary("Set the desktop admin password (no current-password required; localhost-gated).")
        .description(
            "Replaces the admin user's password without requiring the current one. \
             Safe because the localhost-Host middleware that wraps every \
             /api/remote-access/* route already gates this to callers with shell-level \
             access to the desktop machine. The standard \
             /api/users/me/password endpoint with current-password verification \
             remains available for multi-user web deployments.",
        )
        .response::<204, ()>()
        .response_with::<400, (), _>(|r| r.description("New password fails strength check"))
}
