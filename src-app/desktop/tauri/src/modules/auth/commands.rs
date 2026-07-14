//! Auth Tauri Commands
//!
//! Tauri commands for desktop authentication.
//! These are only accessible from the desktop app, not from web.

use crate::modules::backend;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;

/// Response for auto-login command
#[derive(Serialize, Debug)]
pub struct AutoLoginResponse {
    pub user: ziee::User,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // Seconds until token expires
}

/// Mint an admin auto-login response given the server pool + JWT service.
///
/// Extracted from `auto_login` so integration tests can exercise the
/// happy + admin-missing paths without needing to populate the private
/// `JWT_SERVICE` / `SERVER_POOL` OnceLocks inside the backend module.
///
/// Chunk BG-3: the pool is now threaded (from the `ServerBoot` `BootHandle`)
/// instead of reaching the global `ziee::Repos` — `UserRepository::new(pool)` is
/// the same repository `Repos.user` builds from the same pool, so this is
/// behaviour-identical while de-globalizing the desktop consumer surface.
pub async fn mint_admin_login(
    pool: &PgPool,
    jwt_service: &Arc<ziee::JwtService>,
) -> Result<AutoLoginResponse, String> {
    let admin = ziee::UserRepository::new(pool.clone())
        .get_by_username("admin")
        .await
        .map_err(|e| format!("Failed to get admin: {}", e))?
        .ok_or_else(|| "Admin not found - server may still be starting".to_string())?;

    // Same mint path as every server login flow: admin-configured
    // lifetimes + a jti-whitelisted refresh token, so desktop sessions
    // are revocable (logout-everywhere) and pruned like any other.
    let minted = ziee::refresh_tokens::mint_session_tokens(
        pool,
        jwt_service,
        admin.id,
        &admin.username,
        &admin.email,
        admin.is_admin,
    )
    .await
    .map_err(|e| format!("Failed to generate tokens: {}", e))?;
    let tokens = minted.pair;

    Ok(AutoLoginResponse {
        user: admin,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
    })
}

/// Desktop auto-login command
///
/// Returns JWT tokens for the admin user.
/// This is only accessible from the desktop Tauri app.
#[tauri::command]
pub async fn auto_login() -> Result<AutoLoginResponse, String> {
    let jwt_service = backend::get_jwt_service()
        .ok_or_else(|| "Server not ready - JWT service not initialized".to_string())?;
    let pool = backend::get_server_pool()
        .ok_or_else(|| "Server not ready - database pool not initialized".to_string())?;

    let response = mint_admin_login(pool, jwt_service).await?;

    tracing::info!(
        "Desktop auto-login successful for user: {}",
        response.user.username
    );

    Ok(response)
}
