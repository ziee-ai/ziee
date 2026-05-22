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

/// Mark a guide as completed for the current user
#[debug_handler]
pub async fn complete_guide(
    auth: RequirePermissions<(ProfileRead,)>,
    Path(guide_id): Path<String>,
) -> ApiResult<Json<User>> {
    let guide_id = guide_id.trim().to_string();

    if guide_id.is_empty() {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "guide_id cannot be empty").into(),
        );
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

    if gid.is_empty() || sid.is_empty() {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "guide_id and step_id are required")
                .into(),
        );
    }

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
