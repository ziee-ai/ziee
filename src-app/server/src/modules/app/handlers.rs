use aide::transform::TransformOperation;
use axum::{http::StatusCode, Extension, Json};
use axum::debug_handler;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::auth::{JwtService, password, AuthResponse};
use crate::modules::user::UserRepository;

use super::types::{SetupStatusResponse, SetupAdminRequest};
use super::utils::validate_setup_request;

// =====================================================
// Route Handlers
// =====================================================

/// GET /api/app/setup/status
/// Check if initial admin setup is required
#[debug_handler]
pub async fn get_setup_status(
    
) -> ApiResult<Json<SetupStatusResponse>> {
    let user_repo = UserRepository::new(Repos.pool().clone());
    let has_admin = user_repo
        .has_admin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((StatusCode::OK, Json(SetupStatusResponse {
        needs_setup: !has_admin,
        app_name: "Ziee Chat".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })))
}

/// Documentation for get_setup_status endpoint
pub fn get_setup_status_docs(op: TransformOperation) -> TransformOperation {
    op.description("Check if initial admin setup is required")
        .id("App.getSetupStatus")
        .tag("app")
        .response::<200, Json<SetupStatusResponse>>()
}

/// POST /api/app/setup/admin
/// Create the first administrator account
#[debug_handler]
pub async fn setup_admin(
    
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<SetupAdminRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Check if admin already exists
    let user_repo = UserRepository::new(Repos.pool().clone());
    let has_admin = user_repo
        .has_admin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if has_admin {
        return Err((
            StatusCode::FORBIDDEN,
            AppError::forbidden("SETUP_ALREADY_COMPLETE", "Admin user already exists")
        ));
    }

    // Validate input
    validate_setup_request(&req)?;

    // Hash password
    let password_hash = password::hash_password(&req.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::internal_error(format!("Failed to hash password: {}", e))))?;

    // Create admin user with group assignments via repository (handles transaction)
    let user = Repos.app
        .create_admin_user(&req.username, &req.email, &password_hash, req.display_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate tokens
    let tokens = jwt_service
        .generate_tokens(user.id, &user.username, &user.email, user.is_admin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Log setup event
    tracing::info!(
        user_id = %user.id,
        username = %user.username,
        "Admin user created during setup"
    );

    Ok((StatusCode::CREATED, Json(AuthResponse { user, tokens })))
}

/// Documentation for setup_admin endpoint
pub fn setup_admin_docs(op: TransformOperation) -> TransformOperation {
    op.description("Create the first administrator account")
        .id("App.setupAdmin")
        .tag("app")
        .response::<201, Json<AuthResponse>>()
        .response::<403, ()>()
        .response::<400, ()>()
}
