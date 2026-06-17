//! HTTP handlers for the host-mount feature (desktop crate).
//!
//! Scope endpoints are user-scoped: a caller may only read/write mounts on a
//! conversation/project they own (ownership is verified server-side; a
//! mismatch is reported as 404 so existence isn't leaked). Host paths only
//! ever enter via these endpoints — never via the sandbox tool call.

use aide::transform::TransformOperation;
use axum::{debug_handler, extract::Path, http::StatusCode, Json};
use uuid::Uuid;

use ziee::permissions::{with_permission, RequirePermissions};
use ziee::{ApiResult, AppError, Repos};

use super::models::{
    HostMountPolicyResponse, HostMountsBody, MountEntry, UpdateHostMountPolicyRequest,
};
use super::permissions::{HostMountManage, HostMountRead};
use super::repository::HostMountRepository;

fn repo() -> HostMountRepository {
    HostMountRepository::new(Repos.pool().clone())
}

fn ise(e: AppError) -> (StatusCode, AppError) {
    (StatusCode::INTERNAL_SERVER_ERROR, e)
}

async fn ensure_conversation_owner(
    repo: &HostMountRepository,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<(), (StatusCode, AppError)> {
    match repo.conversation_owner(conversation_id).await.map_err(ise)? {
        Some(owner) if owner == user_id => Ok(()),
        _ => Err((StatusCode::NOT_FOUND, AppError::not_found("conversation"))),
    }
}

async fn ensure_project_owner(
    repo: &HostMountRepository,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<(), (StatusCode, AppError)> {
    match repo.project_owner(project_id).await.map_err(ise)? {
        Some(owner) if owner == user_id => Ok(()),
        _ => Err((StatusCode::NOT_FOUND, AppError::not_found("project"))),
    }
}

fn validate_mounts(mounts: &[MountEntry]) -> Result<(), (StatusCode, AppError)> {
    for m in mounts {
        let p = m.host_path.trim();
        if p.is_empty() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                AppError::unprocessable_entity("HOST_MOUNT_EMPTY_PATH", "host_path must not be empty"),
            ));
        }
        if p.contains('\0') {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                AppError::unprocessable_entity(
                    "HOST_MOUNT_BAD_PATH",
                    "host_path must not contain NUL",
                ),
            ));
        }
    }
    Ok(())
}

// ===================== policy =====================

#[debug_handler]
pub async fn get_policy(
    _: RequirePermissions<(HostMountRead,)>,
) -> ApiResult<Json<HostMountPolicyResponse>> {
    let row = repo().get_policy().await.map_err(ise)?;
    Ok((StatusCode::OK, Json(row.into())))
}

pub fn get_policy_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountRead,)>(op)
        .id("HostMount.getPolicy")
        .tag("host-mount")
        .summary("Get the deployment host-mount policy (enabled / allowed prefixes / RW opt-in).")
        .response::<200, Json<HostMountPolicyResponse>>()
}

#[debug_handler]
pub async fn update_policy(
    _: RequirePermissions<(HostMountManage,)>,
    Json(req): Json<UpdateHostMountPolicyRequest>,
) -> ApiResult<Json<HostMountPolicyResponse>> {
    let row = repo().update_policy(&req).await.map_err(ise)?;
    Ok((StatusCode::OK, Json(row.into())))
}

pub fn update_policy_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountManage,)>(op)
        .id("HostMount.updatePolicy")
        .tag("host-mount")
        .summary("Update the deployment host-mount policy.")
        .response::<200, Json<HostMountPolicyResponse>>()
}

// ===================== conversation scope =====================

#[debug_handler]
pub async fn get_conversation_mounts(
    auth: RequirePermissions<(HostMountRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<HostMountsBody>> {
    let repo = repo();
    ensure_conversation_owner(&repo, conversation_id, auth.user.id).await?;
    let mounts = repo
        .conversation_mounts(conversation_id, auth.user.id)
        .await
        .map_err(ise)?
        .unwrap_or_default();
    Ok((StatusCode::OK, Json(HostMountsBody { mounts })))
}

pub fn get_conversation_mounts_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountRead,)>(op)
        .id("HostMount.getConversationMounts")
        .tag("host-mount")
        .summary("List the host folders mounted on a conversation.")
        .response::<200, Json<HostMountsBody>>()
}

#[debug_handler]
pub async fn put_conversation_mounts(
    auth: RequirePermissions<(HostMountManage,)>,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<HostMountsBody>,
) -> ApiResult<Json<HostMountsBody>> {
    let repo = repo();
    ensure_conversation_owner(&repo, conversation_id, auth.user.id).await?;
    validate_mounts(&body.mounts)?;
    repo.upsert_conversation(conversation_id, auth.user.id, &body.mounts)
        .await
        .map_err(ise)?;
    Ok((StatusCode::OK, Json(body)))
}

pub fn put_conversation_mounts_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountManage,)>(op)
        .id("HostMount.putConversationMounts")
        .tag("host-mount")
        .summary("Replace the host folders mounted on a conversation.")
        .response::<200, Json<HostMountsBody>>()
}

// ===================== project scope =====================

#[debug_handler]
pub async fn get_project_mounts(
    auth: RequirePermissions<(HostMountRead,)>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<HostMountsBody>> {
    let repo = repo();
    ensure_project_owner(&repo, project_id, auth.user.id).await?;
    let mounts = repo
        .project_mounts(project_id, auth.user.id)
        .await
        .map_err(ise)?
        .unwrap_or_default();
    Ok((StatusCode::OK, Json(HostMountsBody { mounts })))
}

pub fn get_project_mounts_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountRead,)>(op)
        .id("HostMount.getProjectMounts")
        .tag("host-mount")
        .summary("List the host folders mounted on a project.")
        .response::<200, Json<HostMountsBody>>()
}

#[debug_handler]
pub async fn put_project_mounts(
    auth: RequirePermissions<(HostMountManage,)>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<HostMountsBody>,
) -> ApiResult<Json<HostMountsBody>> {
    let repo = repo();
    ensure_project_owner(&repo, project_id, auth.user.id).await?;
    validate_mounts(&body.mounts)?;
    repo.upsert_project(project_id, auth.user.id, &body.mounts)
        .await
        .map_err(ise)?;
    Ok((StatusCode::OK, Json(body)))
}

pub fn put_project_mounts_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HostMountManage,)>(op)
        .id("HostMount.putProjectMounts")
        .tag("host-mount")
        .summary("Replace the host folders mounted on a project.")
        .response::<200, Json<HostMountsBody>>()
}
