//! Auth Handlers
//!
//! HTTP route handlers for desktop authentication

use serde::Serialize;
use std::sync::Arc;
use ziee_chat::{hash_password, Extension, Json, JwtService, Repos, StatusCode};

/// Response for auto-login endpoint
#[derive(Serialize)]
pub struct AutoLoginResponse {
    pub user: ziee_chat::User,
    pub access_token: String,
    pub refresh_token: String,
}

/// Desktop auto-login handler
/// Creates admin if needed, then returns JWT tokens
pub async fn desktop_auto_login(
    Extension(jwt_service): Extension<Arc<JwtService>>,
) -> Result<Json<AutoLoginResponse>, (StatusCode, String)> {
    // Check if admin exists
    let has_admin = Repos
        .user
        .has_admin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let admin = if !has_admin {
        // Create admin user
        tracing::info!("No admin exists, creating desktop admin user");

        // Hash the placeholder password
        let password_hash = hash_password("desktop-auto-login")
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to hash password: {}", e)))?;

        Repos
            .app
            .create_admin_user("admin", "admin@localhost", &password_hash, None)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create admin: {}", e),
                )
            })?
    } else {
        // Get existing admin by username
        tracing::info!("Admin exists, fetching for auto-login");

        Repos
            .user
            .get_by_username("admin")
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get admin: {}", e),
                )
            })?
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Admin not found".to_string()))?
    };

    // Generate JWT tokens
    let tokens = jwt_service
        .generate_tokens(admin.id, &admin.username, &admin.email, admin.is_admin)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to generate tokens: {}", e),
            )
        })?;

    tracing::info!("Desktop auto-login successful for user: {}", admin.username);

    Ok(Json(AutoLoginResponse {
        user: admin,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    }))
}
