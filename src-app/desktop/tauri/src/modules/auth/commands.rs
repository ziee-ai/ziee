//! Auth Tauri Commands
//!
//! Tauri commands for desktop authentication.
//! These are only accessible from the desktop app, not from web.

use crate::modules::backend;
use serde::Serialize;

/// Response for auto-login command
#[derive(Serialize)]
pub struct AutoLoginResponse {
    pub user: ziee_chat::User,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // Seconds until token expires
}

/// Desktop auto-login command
///
/// Returns JWT tokens for the admin user.
/// This is only accessible from the desktop Tauri app.
#[tauri::command]
pub async fn auto_login() -> Result<AutoLoginResponse, String> {
    // Get JWT service from global state
    let jwt_service = backend::get_jwt_service()
        .ok_or_else(|| "Server not ready - JWT service not initialized".to_string())?;

    // Get admin user (should exist from startup)
    let admin = ziee_chat::Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| format!("Failed to get admin: {}", e))?
        .ok_or_else(|| "Admin not found - server may still be starting".to_string())?;

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(admin.id, &admin.username, &admin.email, admin.is_admin)
        .map_err(|e| format!("Failed to generate tokens: {}", e))?;

    tracing::info!(
        "Desktop auto-login successful for user: {}",
        admin.username
    );

    Ok(AutoLoginResponse {
        user: admin,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
    })
}
