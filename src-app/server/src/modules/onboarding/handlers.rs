// Onboarding handlers

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use uuid::Uuid;

use super::models::OnboardingProgress;
use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        auth::jwt_extractor::JwtAuth,
        permissions::{RequirePermissions, with_permission},
        user::permissions::ProfileEdit,
    },
};

/// Maximum length of a guide_id / step_id we'll accept. Closes 13-misc H-2
/// (High) — without this, any authenticated user with the default
/// profile::read permission could spam-append arbitrary-length entries
/// into completed_guide_ids / completed_step_ids, causing self-DoS,
/// row-bloat, and egress amplification.
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

/// Parse the user id out of the JWT claims.
/// Owner-scoped notify so the user's other devices refetch onboarding
/// progress (a guide/step completed on one device shouldn't keep showing on
/// another). Shared by `complete_guide` and `complete_guide_step`.
fn notify_onboarding_updated(user_id: Uuid, origin: Option<Uuid>) {
    crate::modules::sync::publish(
        crate::modules::sync::SyncEntity::Onboarding,
        crate::modules::sync::SyncAction::Update,
        user_id,
        crate::modules::sync::Audience::owner(user_id),
        origin,
    );
}

fn user_id_from_claims(auth: &JwtAuth) -> Result<Uuid, AppError> {
    Uuid::parse_str(&auth.claims.sub)
        .map_err(|e| AppError::internal_error(format!("Invalid user ID in token: {}", e)))
}

/// Get the current user's onboarding progress. Authentication-only gate
/// (the user reads their own per-user state), mirroring GET /auth/me.
#[debug_handler]
pub async fn get_progress(auth: JwtAuth) -> ApiResult<Json<OnboardingProgress>> {
    let user_id = user_id_from_claims(&auth)?;
    let progress = Repos.onboarding.get_progress(user_id).await?;
    Ok((StatusCode::OK, Json(progress)))
}

pub fn get_progress_docs(op: TransformOperation) -> TransformOperation {
    op.id("Onboarding.getProgress")
        .tag("Onboarding")
        .summary("Get the current user's onboarding progress")
        .response::<200, Json<OnboardingProgress>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Mark a guide as completed for the current user
#[debug_handler]
pub async fn complete_guide(
    auth: RequirePermissions<(ProfileEdit,)>,
    Path(guide_id): Path<String>,
    origin: crate::modules::sync::SyncOrigin,
) -> ApiResult<Json<OnboardingProgress>> {
    let guide_id = guide_id.trim().to_string();

    if !is_valid_onboarding_id(&guide_id) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "guide_id must be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }

    // Cardinality cap (13-misc H-2) — read current progress from the
    // dedicated table now that it no longer rides on `User`.
    let current = Repos.onboarding.get_progress(auth.user.id).await?;
    if current.completed_guide_ids.len() >= MAX_ONBOARDING_COMPLETIONS {
        return Err(AppError::bad_request(
            "ONBOARDING_LIMIT",
            "Maximum number of completed onboarding guides reached",
        )
        .into());
    }

    let progress = Repos
        .onboarding
        .complete_guide(auth.user.id, &guide_id, MAX_ONBOARDING_COMPLETIONS as i32)
        .await?;

    notify_onboarding_updated(auth.user.id, origin.0);

    Ok((StatusCode::OK, Json(progress)))
}

pub fn complete_guide_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileEdit,)>(op)
        .id("Onboarding.complete")
        .tag("Onboarding")
        .summary("Mark a guide as completed")
        .response::<200, Json<OnboardingProgress>>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Mark a guide step as completed for the current user
#[debug_handler]
pub async fn complete_guide_step(
    auth: RequirePermissions<(ProfileEdit,)>,
    origin: crate::modules::sync::SyncOrigin,
    Path((guide_id, step_id)): Path<(String, String)>,
) -> ApiResult<Json<OnboardingProgress>> {
    let gid = guide_id.trim().to_string();
    let sid = step_id.trim().to_string();

    if !is_valid_onboarding_id(&gid) || !is_valid_onboarding_id(&sid) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "guide_id and step_id must each be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }

    let current = Repos.onboarding.get_progress(auth.user.id).await?;
    if current.completed_step_ids.len() >= MAX_ONBOARDING_COMPLETIONS {
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
    let progress = Repos
        .onboarding
        .complete_guide_step(auth.user.id, &step_key, MAX_ONBOARDING_COMPLETIONS as i32)
        .await?;

    notify_onboarding_updated(auth.user.id, origin.0);

    Ok((StatusCode::OK, Json(progress)))
}

pub fn complete_guide_step_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileEdit,)>(op)
        .id("Onboarding.completeStep")
        .tag("Onboarding")
        .summary("Mark a guide step as completed")
        .response::<200, Json<OnboardingProgress>>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

#[cfg(test)]
mod tests {
    use super::{is_valid_onboarding_id, MAX_ONBOARDING_ID_LEN};

    #[test]
    fn valid_slug_ids_are_accepted() {
        for id in ["getting-started", "memory-setup", "step_1", "a", "abc-123_xyz"] {
            assert!(is_valid_onboarding_id(id), "{id} should be valid");
        }
    }

    #[test]
    fn empty_id_is_rejected() {
        assert!(!is_valid_onboarding_id(""));
    }

    #[test]
    fn id_at_max_len_ok_one_over_rejected() {
        let at_max = "a".repeat(MAX_ONBOARDING_ID_LEN);
        let over = "a".repeat(MAX_ONBOARDING_ID_LEN + 1);
        assert!(is_valid_onboarding_id(&at_max), "len==MAX must be accepted");
        assert!(!is_valid_onboarding_id(&over), "len>MAX must be rejected");
    }

    #[test]
    fn disallowed_chars_are_rejected() {
        // uppercase, slash (step_key separator collision), spaces, dots, NUL,
        // control chars, and non-ascii must all be refused.
        for bad in [
            "Getting-Started", // uppercase
            "a/b",             // slash separator collision
            "a b",             // space
            "a.b",             // dot
            "a\0b",            // NUL byte
            "a\nb",            // control char
            "café",            // non-ascii
            "guide!",          // punctuation
        ] {
            assert!(!is_valid_onboarding_id(bad), "{bad:?} should be rejected");
        }
    }
}
