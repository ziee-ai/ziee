// Onboarding handlers

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        permissions::{RequirePermissions, with_permission},
        user::{models::User, permissions::ProfileRead},
    },
};

/// Maximum length of a guide_id / step_id we'll accept. Closes 13-misc H-2
/// (High) — without this, any authenticated user with the default
/// profile::read permission could spam-append arbitrary-length entries
/// into completed_onboarding_ids / completed_onboarding_step_ids,
/// causing self-DoS, row-bloat, and O(n^2) egress amplification on
/// /api/auth/me.
const MAX_ONBOARDING_ID_LEN: usize = 64;

/// Maximum number of completed-guide entries we'll allow per user.
/// Same finding — caps the array cardinality.
const MAX_ONBOARDING_COMPLETIONS: usize = 256;

/// Allowed characters in a guide_id / step_id. We accept lowercase letters,
/// digits, dash, underscore — typical slug-style identifiers. This
/// blocks NUL bytes, '/', control chars, and the '/' separator collision
/// in the step_key concatenation flagged by the audit (L-class).
fn is_valid_onboarding_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ONBOARDING_ID_LEN
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
        })
}

/// Mark a guide as completed for the current user
#[debug_handler]
pub async fn complete_guide(
    auth: RequirePermissions<(ProfileRead,)>,
    Path(guide_id): Path<String>,
) -> ApiResult<Json<User>> {
    let guide_id = guide_id.trim().to_string();

    if !is_valid_onboarding_id(&guide_id) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "guide_id must be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }

    if auth.user.completed_onboarding_ids.len() >= MAX_ONBOARDING_COMPLETIONS {
        return Err(AppError::bad_request(
            "ONBOARDING_LIMIT",
            "Maximum number of completed onboarding guides reached",
        )
        .into());
    }

    let user = Repos
        .user
        .complete_guide(auth.user.id, &guide_id)
        .await?;

    Ok((StatusCode::OK, Json(user)))
}

pub fn complete_guide_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("Onboarding.complete")
        .tag("Onboarding")
        .summary("Mark a guide as completed")
        .response::<200, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Mark a guide step as completed for the current user
#[debug_handler]
pub async fn complete_guide_step(
    auth: RequirePermissions<(ProfileRead,)>,
    Path((guide_id, step_id)): Path<(String, String)>,
) -> ApiResult<Json<User>> {
    let gid = guide_id.trim().to_string();
    let sid = step_id.trim().to_string();

    if !is_valid_onboarding_id(&gid) || !is_valid_onboarding_id(&sid) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "guide_id and step_id must each be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }

    if auth.user.completed_onboarding_step_ids.len() >= MAX_ONBOARDING_COMPLETIONS {
        return Err(AppError::bad_request(
            "ONBOARDING_LIMIT",
            "Maximum number of completed onboarding steps reached",
        )
        .into());
    }

    // gid/sid charset is restricted by is_valid_onboarding_id to
    // [a-z0-9_-], so the '/' separator cannot collide with content
    // inside the components.
    let step_key = format!("{}/{}", gid, sid);
    let user = Repos
        .user
        .complete_guide_step(auth.user.id, &step_key)
        .await?;

    Ok((StatusCode::OK, Json(user)))
}

pub fn complete_guide_step_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("Onboarding.completeStep")
        .tag("Onboarding")
        .summary("Mark a guide step as completed")
        .response::<200, Json<User>>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
