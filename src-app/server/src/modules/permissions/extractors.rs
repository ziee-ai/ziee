// Permission extractors

use std::{marker::PhantomData, sync::Arc};

use aide::OperationIo;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use crate::{
    common::AppError,
    core::Repos,
    modules::{
        auth::jwt::JwtService,
        user::models::{Group, User},
    },
};

use super::{checker::check_permission_union, types::PermissionList};

/// Shared auth pipeline: validate the JWT, load the user, check active status.
/// Used by both `RequirePermissions` and `RequireAdmin` to avoid duplication.
async fn extract_authenticated_user(
    parts: &mut Parts,
) -> Result<User, (StatusCode, AppError)> {
    // Get JWT service from app state
    let jwt_service = parts.extensions.get::<Arc<JwtService>>().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error("JWT service not configured"),
        )
    })?;

    // Extract Authorization header
    let auth_header = parts
        .headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("MISSING_TOKEN", "Authorization header is missing"),
            )
        })?;

    // Extract and validate token
    let token = JwtService::extract_token_from_header(auth_header)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    let claims = jwt_service
        .validate_access_token(token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    // Parse user ID from claims
    let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error("Invalid user ID in token"),
        )
    })?;

    // Load user from database using global Repos
    let user = Repos
        .user
        .get_by_id(user_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Failed to load user: {}", e)),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                AppError::unauthorized("USER_NOT_FOUND", "User not found"),
            )
        })?;

    // Check if user is active
    if !user.is_active {
        return Err((
            StatusCode::FORBIDDEN,
            AppError::forbidden("USER_INACTIVE", "User account is inactive"),
        ));
    }

    Ok(user)
}

// =====================================================
// RequirePermissions - Generic Permission Extractor
// =====================================================

/// Generic permission extractor for checking user permissions
///
/// Supports single or multiple permissions using tuple syntax:
/// - Single: `RequirePermissions<(UsersRead,)>`
/// - Multiple: `RequirePermissions<(UsersRead, UsersEdit)>`
///
/// When multiple permissions are specified, the user must have ALL of them (AND logic).
#[derive(Clone, OperationIo)]
#[aide(input)]
pub struct RequirePermissions<Perms: PermissionList> {
    pub user: User,
    pub groups: Vec<Group>,
    _marker: PhantomData<Perms>,
}

impl<Perms: PermissionList> FromRequestParts<()> for RequirePermissions<Perms> {
    type Rejection = (StatusCode, AppError);

    fn from_request_parts(
        parts: &mut Parts,
        _state: &(),
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let user = extract_authenticated_user(parts).await?;

            // Root admin bypass - is_admin always has full access
            if user.is_admin {
                return Ok(Self {
                    user,
                    groups: vec![],
                    _marker: PhantomData,
                });
            }

            // Load user's groups with permissions using global Repos
            let groups = Repos.user.get_user_groups(user.id).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error(format!("Failed to load groups: {}", e)),
                )
            })?;

            // Check if user has ALL required permissions via union (AND logic)
            let required_permissions = Perms::permissions();
            let missing_permissions: Vec<&str> = required_permissions
                .iter()
                .filter(|&&perm| !check_permission_union(&user, &groups, perm))
                .copied()
                .collect();

            if !missing_permissions.is_empty() {
                let error_message = if missing_permissions.len() == 1 {
                    format!("Missing required permission: {}", missing_permissions[0])
                } else {
                    format!(
                        "Missing required permissions: {}",
                        missing_permissions.join(", ")
                    )
                };

                return Err((
                    StatusCode::FORBIDDEN,
                    AppError::forbidden("INSUFFICIENT_PERMISSIONS", error_message),
                ));
            }

            Ok(Self {
                user,
                groups,
                _marker: PhantomData,
            })
        }
    }
}

// =====================================================
// RequireAdmin - Root Admin Only Extractor
// =====================================================

/// Extractor that requires root admin (is_admin = true)
/// Use this for operations that should ONLY be available to the root admin
// Real axum extractor API (impl below) paralleling RequirePermissions; no route
// uses root-admin-only gating yet. Narrow allow (was module blanket) rather
// than delete a public extractor surface.
#[allow(dead_code)]
#[derive(Clone, OperationIo)]
#[aide(input)]
pub struct RequireAdmin {
    pub user: User,
}

impl FromRequestParts<()> for RequireAdmin {
    type Rejection = (StatusCode, AppError);

    fn from_request_parts(
        parts: &mut Parts,
        _state: &(),
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let user = extract_authenticated_user(parts).await?;

            // Check if user is root admin
            if !user.is_admin {
                return Err((
                    StatusCode::FORBIDDEN,
                    AppError::forbidden("ADMIN_REQUIRED", "Root administrator access required"),
                ));
            }

            Ok(Self { user })
        }
    }
}
