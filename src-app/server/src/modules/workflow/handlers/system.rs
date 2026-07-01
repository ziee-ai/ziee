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
use crate::modules::workflow::types::{
    GroupSystemWorkflowsResponse, UpdateGroupSystemWorkflowsRequest, WorkflowGroupsRequest,
    WorkflowListResponse,
};

pub async fn list_system_workflows(
    _auth: RequirePermissions<(WorkflowsManageSystem,)>,
) -> ApiResult<Json<WorkflowListResponse>> {
    // Admin moderation + group-assignment picker surface: ALL system
    // workflows, unconditionally. Must NOT use the group-access-filtered
    // `list_for_user` (with nil user that hides any system workflow already
    // assigned to a group). Mirrors skill's `list_system`.
    let workflows =
        repository::list_system(Repos.pool(), crate::common::PAGINATION_MAX_PER_PAGE as i64, 0)
            .await?;
    Ok((StatusCode::OK, Json(WorkflowListResponse { workflows })))
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

// ============================================================
// Group-centric assignment (User Groups page widget)
// The reverse of get/set_workflow_groups: given a group, list/replace the
// system workflows assigned to it. Mirrors MCP's get/update_group_system_servers.
// ============================================================

/// Get all system workflows assigned to a group (for the User Groups widget).
pub async fn get_group_system_workflows(
    _auth: RequirePermissions<(WorkflowsAssignToGroups,)>,
    AxumPath(group_id): AxumPath<Uuid>,
) -> ApiResult<Json<GroupSystemWorkflowsResponse>> {
    let workflows = repository::get_system_workflows_for_group(Repos.pool(), group_id).await?;
    Ok((StatusCode::OK, Json(GroupSystemWorkflowsResponse { workflows })))
}

pub fn get_group_system_workflows_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsAssignToGroups,)>(op)
        .id("Group.getSystemWorkflows")
        .tag("Admin - Groups")
        .summary("Get all system workflows assigned to a group")
        .description("Get all system workflows assigned to a group (for the User Groups assignment widget)")
        .response::<200, Json<GroupSystemWorkflowsResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

/// Atomically replace the set of system workflows assigned to a group. Rejects
/// non-system / unknown workflow ids with a 400 BEFORE writing (the
/// `group_workflows` trigger would otherwise surface as a 500).
pub async fn update_group_system_workflows(
    _auth: RequirePermissions<(WorkflowsAssignToGroups,)>,
    AxumPath(group_id): AxumPath<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateGroupSystemWorkflowsRequest>,
) -> ApiResult<Json<GroupSystemWorkflowsResponse>> {
    use std::collections::HashSet;

    let new_ids: HashSet<Uuid> = request.workflow_ids.iter().copied().collect();

    // Guard: every requested id must be an existing system-scope workflow.
    if !new_ids.is_empty() {
        let system_count = repository::count_system_workflows_in(
            Repos.pool(),
            &new_ids.iter().copied().collect::<Vec<_>>(),
        )
        .await?;
        if system_count as usize != new_ids.len() {
            return Err::<_, (StatusCode, AppError)>(
                AppError::bad_request(
                    "INVALID_SCOPE",
                    "only existing system-scope workflows can be assigned to groups",
                )
                .into(),
            );
        }
    }

    let current = repository::get_system_workflows_for_group(Repos.pool(), group_id).await?;
    let current_ids: HashSet<Uuid> = current.iter().map(|w| w.id).collect();

    let to_add: Vec<Uuid> = new_ids.difference(&current_ids).copied().collect();
    let to_remove: Vec<Uuid> = current_ids.difference(&new_ids).copied().collect();

    let mut affected: HashSet<Uuid> = HashSet::new();
    affected.extend(to_add.iter().copied());
    affected.extend(to_remove.iter().copied());

    for workflow_id in to_remove {
        repository::remove_workflow_group(Repos.pool(), workflow_id, group_id).await?;
    }
    for workflow_id in to_add {
        repository::assign_workflow_to_group(Repos.pool(), workflow_id, group_id).await?;
    }

    for workflow_id in affected {
        crate::modules::workflow::events::emit_system_workflow(
            SyncAction::Update,
            workflow_id,
            origin.0,
        );
    }

    let workflows = repository::get_system_workflows_for_group(Repos.pool(), group_id).await?;
    Ok((StatusCode::OK, Json(GroupSystemWorkflowsResponse { workflows })))
}

pub fn update_group_system_workflows_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsAssignToGroups,)>(op)
        .id("Group.updateSystemWorkflows")
        .tag("Admin - Groups")
        .summary("Update system workflows assigned to a group")
        .description("Atomically updates system-workflow assignments. Adds new workflows and removes unspecified ones.")
        .response::<200, Json<GroupSystemWorkflowsResponse>>()
        .response_with::<400, (), _>(|r| r.description("Bad request — non-system or unknown workflow id"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}
