//! Auth Handlers
//!
//! HTTP route handlers for desktop authentication

use schemars::JsonSchema;
use serde::Serialize;
use std::sync::Arc;
use ziee_chat::{Extension, Json, JwtService, Repos, StatusCode, TransformOperation};

/// Response for auto-login endpoint
#[derive(Serialize, JsonSchema)]
pub struct AutoLoginResponse {
    pub user: ziee_chat::User,
    pub access_token: String,
    pub refresh_token: String,
}

/// OpenAPI documentation for desktop_auto_login endpoint
pub fn desktop_auto_login_docs(op: TransformOperation) -> TransformOperation {
    op.description("Auto-login for desktop app. Creates admin account on first run.")
        .id("DesktopAuth.autoLogin")
        .tag("desktop-auth")
        .response::<200, Json<AutoLoginResponse>>()
}

/// Desktop auto-login handler
/// Returns JWT tokens for the admin user (created at startup)
pub async fn desktop_auto_login(
    Extension(jwt_service): Extension<Arc<JwtService>>,
) -> Result<Json<AutoLoginResponse>, (StatusCode, String)> {
    // Get admin user (should exist from startup)
    let admin = Repos
        .user
        .get_by_username("admin")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get admin: {}", e),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Admin not found - server may still be starting".to_string(),
            )
        })?;

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
