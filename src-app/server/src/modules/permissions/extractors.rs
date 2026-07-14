// Permission extractors
//
// Chunk B3: the generic `RequirePermissions` / `RequireAdmin` extractors moved
// into `ziee_framework::permissions`, generic over an injected
// `IdentityResolver` so the framework enforcement path never names ziee's global
// `Repos`/`JwtService`/`User`/`Group`. This module now:
//   1. defines `ZieeIdentityResolver` — ziee's concrete resolver, backed by the
//      global `Repos` + the `Arc<JwtService>` already layered into extensions
//      (the former `extract_authenticated_user` body + `get_user_groups`, kept
//      byte-identical), and
//   2. re-exports the moved extractors as equivalence-preserving type aliases
//      fixing the resolver to `ZieeIdentityResolver`, so every
//      `RequirePermissions<(UsersRead,)>` call site is unchanged.
// ziee installs `Arc<ZieeIdentityResolver>` into the request extensions at
// startup (main.rs + lib.rs, alongside the JWT service).

use std::sync::Arc;

use axum::http::{StatusCode, request::Parts};

use ziee_framework::permissions::IdentityResolver;

use crate::{
    common::AppError,
    core::Repos,
    modules::{
        auth::jwt::JwtService,
        user::models::{Group, User},
    },
};

/// ziee's concrete identity resolver: validates the JWT (read from the request
/// extensions) and loads the acting `User` + `Group`s from the global `Repos`.
/// A zero-sized unit installed into the request extensions at startup.
#[derive(Clone, Copy, Default)]
pub struct ZieeIdentityResolver;

#[async_trait::async_trait]
impl IdentityResolver for ZieeIdentityResolver {
    type User = User;
    type Group = Group;

    /// Validate the JWT, load the user, and check active status. Byte-identical
    /// to the former `extract_authenticated_user`.
    async fn authenticate(&self, parts: &mut Parts) -> Result<User, (StatusCode, AppError)> {
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
                    AppError::database_error(e),
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

    /// Load the user's groups with permissions using the global `Repos`.
    async fn load_groups(&self, user: &User) -> Result<Vec<Group>, (StatusCode, AppError)> {
        Repos.user.get_user_groups(user.id).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::database_error(e),
            )
        })
    }

    /// A group's permissions IFF it is active — mirrors the `group.is_active`
    /// guard the former `check_permission_union` applied inside the extractor.
    fn active_group_permissions(group: &Group) -> Option<&[String]> {
        if group.is_active {
            Some(&group.permissions)
        } else {
            None
        }
    }
}

/// Generic permission extractor, fixed to ziee's resolver. See
/// [`ziee_framework::permissions::RequirePermissions`] for the enforcement
/// logic. Supports single or multiple permissions via tuple syntax
/// (`RequirePermissions<(UsersRead,)>` / `RequirePermissions<(UsersRead, UsersEdit)>`,
/// ALL-of AND logic).
pub type RequirePermissions<Perms> =
    ziee_framework::permissions::RequirePermissions<ZieeIdentityResolver, Perms>;

/// Root-admin-only extractor, fixed to ziee's resolver. See
/// [`ziee_framework::permissions::RequireAdmin`]. No route uses root-admin-only
/// gating yet (the former struct carried the same `#[allow(dead_code)]`).
#[allow(dead_code)]
pub type RequireAdmin = ziee_framework::permissions::RequireAdmin<ZieeIdentityResolver>;
