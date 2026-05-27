//! Auth Tauri Commands
//!
//! Tauri commands for desktop authentication.
//! These are only accessible from the desktop app, not from web.

use crate::modules::backend;
use serde::Serialize;
use std::sync::Arc;

/// Response for auto-login command
#[derive(Serialize, Debug)]
pub struct AutoLoginResponse {
    pub user: ziee::User,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // Seconds until token expires
}

/// Mint an admin auto-login response given a JWT service.
///
/// Extracted from `auto_login` so integration tests can exercise the
/// happy + admin-missing paths without needing to populate the private
/// `JWT_SERVICE` OnceLock inside the backend module.
pub async fn mint_admin_login(
    jwt_service: &Arc<ziee::JwtService>,
) -> Result<AutoLoginResponse, String> {
    let admin = ziee::Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| format!("Failed to get admin: {}", e))?
        .ok_or_else(|| "Admin not found - server may still be starting".to_string())?;

    let tokens = jwt_service
        .generate_tokens(admin.id, &admin.username, &admin.email, admin.is_admin)
        .map_err(|e| format!("Failed to generate tokens: {}", e))?;

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

    let response = mint_admin_login(jwt_service).await?;

    tracing::info!(
        "Desktop auto-login successful for user: {}",
        response.user.username
    );

    Ok(response)
}
