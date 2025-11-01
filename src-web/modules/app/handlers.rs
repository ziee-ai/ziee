use axum::{extract::State, http::StatusCode, Extension, Json};
use axum::debug_handler;
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::{ApiResult, AppError};
use crate::modules::auth::{JwtService, password, AuthResponse};
use crate::modules::user::{User, UserRepository};

use super::types::{SetupStatusResponse, SetupAdminRequest};
use super::utils::validate_setup_request;

// =====================================================
// Route Handlers
// =====================================================

/// GET /api/app/setup/status
/// Check if initial admin setup is required
#[debug_handler]
pub async fn get_setup_status(
    State(pool): State<PgPool>,
) -> ApiResult<Json<SetupStatusResponse>> {
    let user_repo = UserRepository::new(pool);
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

/// POST /api/app/setup/admin
/// Create the first administrator account
#[debug_handler]
pub async fn setup_admin(
    State(pool): State<PgPool>,
    Extension(jwt_service): Extension<Arc<JwtService>>,
    Json(req): Json<SetupAdminRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Check if admin already exists
    let user_repo = UserRepository::new(pool.clone());
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

    // Begin transaction
    let mut tx = pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    // Double-check within transaction (race condition protection)
    let admin_exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE is_admin = true) as "exists!""#
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    if admin_exists {
        tx.rollback().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;
        return Err((
            StatusCode::FORBIDDEN,
            AppError::forbidden("SETUP_ALREADY_COMPLETE", "Admin user already exists")
        ));
    }

    // Create admin user
    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (username, email, password_hash, display_name, is_active, is_admin)
        VALUES ($1, $2, $3, $4, true, true)
        RETURNING id, username, email, email_verified, password_hash, display_name,
                  avatar_url, is_active, is_admin, permissions,
                  created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
        "#,
        req.username,
        req.email,
        password_hash,
        req.display_name
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    // Assign to Administrators group
    let admin_group = sqlx::query!(
        r#"SELECT id FROM groups WHERE name = 'Administrators' LIMIT 1"#
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    sqlx::query!(
        r#"INSERT INTO user_groups (user_id, group_id) VALUES ($1, $2)"#,
        user.id,
        admin_group.id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

    // Commit transaction
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, AppError::database_error(e)))?;

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
