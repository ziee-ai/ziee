//! Admin-scope workflow handlers (`/api/workflows/system/*`).
//!
//! B4 ships LIST + GET + DELETE. Install-from-hub re-binds to
//! `hub::handlers::create_system_workflow_from_hub`. Group-assignment
//! endpoints land in B6.

#![allow(dead_code)]

use aide::transform::TransformOperation;
use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::with_permission;
use crate::modules::workflow::models::Workflow;
use crate::modules::workflow::permissions::WorkflowsManageSystem;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::WorkflowListResponse;

pub async fn list_system_workflows(
    _auth: RequirePermissions<(WorkflowsManageSystem,)>,
) -> ApiResult<Json<WorkflowListResponse>> {
    // System workflows are visible to everyone; admin list is the
    // moderation surface (delete-only here).
    let workflows = repository::list_for_user(Repos.pool(), Uuid::nil()).await?;
    let only_system: Vec<_> = workflows.into_iter().filter(|w| w.scope == "system").collect();
    Ok((StatusCode::OK, Json(WorkflowListResponse { workflows: only_system })))
}

pub fn list_system_workflows_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManageSystem,)>(op)
        .id("Workflow.listSystem")
        .tag("Workflows - Admin")
        .summary("List system-scope workflows")
        .response::<200, Json<WorkflowListResponse>>()
}

pub async fn get_system_workflow(
    _auth: RequirePermissions<(WorkflowsManageSystem,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<Json<Workflow>> {
    let wf = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if wf.scope != "system" {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_NOT_SYSTEM",
            "workflow is not system-scope",
        )).into());
    }
    Ok((StatusCode::OK, Json(wf)))
}

pub fn get_system_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManageSystem,)>(op)
        .id("Workflow.getSystem")
        .tag("Workflows - Admin")
        .summary("Get a system-scope workflow")
        .response::<200, Json<Workflow>>()
}

pub async fn delete_system_workflow(
    _auth: RequirePermissions<(WorkflowsManageSystem,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<()>  {
    let wf = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if wf.scope != "system" {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_NOT_SYSTEM",
            "use user-scope delete for non-system workflows",
        )).into());
    }
    repository::delete(Repos.pool(), id).await?;
    crate::modules::workflow::events::emit_system_workflow(
        crate::modules::sync::SyncAction::Delete,
        id,
        None,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_system_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManageSystem,)>(op)
        .id("Workflow.deleteSystem")
        .tag("Workflows - Admin")
        .summary("Delete a system-scope workflow")
        .response::<204, ()>()
}

// Install from hub is exposed under /api/hub/workflows/* (see hub
// module routes); the workflow module doesn't re-bind it because the
// existing handlers require Extension<EventBus> + SyncOrigin that are
// awkward to thread through a re-bind shim.
