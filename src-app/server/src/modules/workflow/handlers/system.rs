//! Admin-scope workflow handlers (`/api/workflows/system/*`).
//!
//! LIST + GET + UPDATE + DELETE + install-from-hub re-bind + multipart
//! import + group assignment. Mirrors `skill::handlers` (system half).

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
use crate::modules::sync::{SyncAction, SyncOrigin};
use crate::modules::workflow::models::Workflow;
use crate::modules::workflow::permissions::{WorkflowsAssignToGroups, WorkflowsManageSystem};
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{WorkflowGroupsRequest, WorkflowListResponse};

pub async fn list_system_workflows(
    _auth: RequirePermissions<(WorkflowsManageSystem,)>,
) -> ApiResult<Json<WorkflowListResponse>> {
    // System workflows are visible to everyone; admin list is the
    // moderation surface (delete-only here).
    let workflows = repository::list_for_user(
        Repos.pool(),
        Uuid::nil(),
        crate::common::DEFAULT_PAGE_SIZE as i64,
        0,
    )
    .await?;
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
    // Best-effort rm the extracted bundle dir FIRST (mirrors skill +
    // user-scope workflow delete). Ignore NotFound.
    super::remove_extracted_dir(&wf.extracted_path).await;
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

// ============================================================
// Group assignment (mirrors skill::handlers group endpoints)
// ============================================================

pub async fn get_workflow_groups(
    _auth: RequirePermissions<(WorkflowsAssignToGroups,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<Json<Vec<Uuid>>> {
    let groups = repository::get_workflow_groups(Repos.pool(), id).await?;
    Ok((StatusCode::OK, Json(groups)))
}

pub fn get_workflow_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsAssignToGroups,)>(op)
        .id("WorkflowSystem.getGroups")
        .tag("Workflows - Admin")
        .summary("Get groups assigned to a system-scope workflow")
        .response::<200, Json<Vec<Uuid>>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

pub async fn set_workflow_groups(
    _auth: RequirePermissions<(WorkflowsAssignToGroups,)>,
    AxumPath(id): AxumPath<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<WorkflowGroupsRequest>,
) -> ApiResult<StatusCode> {
    let existing = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    // Only system-scope workflows can be assigned to groups (matches the
    // group_workflows trigger; surface a 400 before hitting it).
    if existing.scope != "system" {
        return Err::<_, (StatusCode, AppError)>(
            AppError::bad_request(
                "INVALID_SCOPE",
                "only system-scope workflows can be assigned to groups",
            )
            .into(),
        );
    }
    repository::set_workflow_groups(Repos.pool(), id, &request.group_ids).await?;
    crate::modules::workflow::events::emit_system_workflow(SyncAction::Update, id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn set_workflow_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsAssignToGroups,)>(op)
        .id("WorkflowSystem.setGroups")
        .tag("Workflows - Admin")
        .summary("Replace the set of groups assigned to a workflow")
        .response_with::<204, (), _>(|r| r.description("Assignments updated"))
        .response_with::<400, (), _>(|r| r.description("Bad request — non-system scope"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Workflow not found"))
}

pub async fn remove_workflow_group(
    _auth: RequirePermissions<(WorkflowsAssignToGroups,)>,
    AxumPath((id, group_id)): AxumPath<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    repository::remove_workflow_group(Repos.pool(), id, group_id).await?;
    crate::modules::workflow::events::emit_system_workflow(SyncAction::Update, id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn remove_workflow_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsAssignToGroups,)>(op)
        .id("WorkflowSystem.removeFromGroup")
        .tag("Workflows - Admin")
        .summary("Remove a workflow from one group")
        .response_with::<204, (), _>(|r| r.description("Removed"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}
