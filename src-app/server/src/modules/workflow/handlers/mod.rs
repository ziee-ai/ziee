//! Workflow REST handlers (user + admin split; B4 + B6).
//!
//! User: list / get / delete + install-from-hub re-bind + RUN + CANCEL.
//! Admin (`/system/*`): list / delete + group assignment.
//! `/validate`, `/import`, `/dry-run`, `/test` (B6) live in `dev.rs`.

#![allow(dead_code)]

pub mod dev;
pub mod system;

use aide::transform::TransformOperation;
use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::with_permission;
use crate::modules::sync::{SyncAction, SyncOrigin};
use crate::modules::workflow::models::{UpdateWorkflow, Workflow, WorkflowRun};
use crate::modules::workflow::permissions::{
    WorkflowsExecute, WorkflowsManage, WorkflowsRead,
};
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{
    WorkflowListResponse, WorkflowRunRequest, WorkflowRunStartResponse,
};

/// Re-export wrapper: `POST /api/workflows/install-from-hub` (user) +
/// `POST /api/workflows/system/install-from-hub` (admin) bind the
/// existing hub handlers at the canonical workflow-facing paths. Single
/// implementation in `hub::handlers` — same compiled function bound to a
/// second route (mirrors `skill::handlers`'s install re-export).
pub use crate::modules::hub::handlers::{
    create_system_workflow_from_hub as install_system_from_hub,
    create_system_workflow_from_hub_docs as install_system_from_hub_docs,
    create_workflow_from_hub as install_from_hub,
    create_workflow_from_hub_docs as install_from_hub_docs,
};

// ============================================================
// List + Get + Delete
// ============================================================

pub async fn list_user_workflows(
    auth: RequirePermissions<(WorkflowsRead,)>,
) -> ApiResult<Json<WorkflowListResponse>> {
    let workflows = repository::list_for_user(Repos.pool(), auth.user.id).await?;
    Ok((StatusCode::OK, Json(WorkflowListResponse { workflows })))
}

pub fn list_user_workflows_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.list")
        .tag("Workflows")
        .summary("List workflows visible to the current user")
        .response::<200, Json<WorkflowListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

pub async fn get_user_workflow(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<Json<Workflow>> {
    let wf = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if wf.scope == "user"
        && wf.owner_user_id != Some(auth.user.id)
    {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_FORBIDDEN",
            "workflow owned by another user",
        )).into());
    }
    Ok((StatusCode::OK, Json(wf)))
}

pub fn get_user_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.get")
        .tag("Workflows")
        .summary("Get a workflow by id")
        .response::<200, Json<Workflow>>()
}

pub async fn delete_user_workflow(
    auth: RequirePermissions<(WorkflowsManage,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<()>  {
    let wf = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if wf.scope != "user" || wf.owner_user_id != Some(auth.user.id) {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_FORBIDDEN",
            "cannot delete non-owned workflow",
        )).into());
    }
    // Best-effort rm the extracted bundle dir FIRST (mirrors skill
    // delete — the bundle dir is per-install, not per-run). Ignore
    // NotFound so a re-delete / already-cleaned dir is not an error.
    remove_extracted_dir(&wf.extracted_path).await;
    repository::delete(Repos.pool(), id).await?;
    crate::modules::workflow::events::emit_user_workflow(
        crate::modules::sync::SyncAction::Delete,
        id,
        auth.user.id,
        None,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_user_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManage,)>(op)
        .id("Workflow.delete")
        .tag("Workflows")
        .summary("Delete a user-owned workflow")
        .response::<204, ()>()
}

/// Edit a user-owned workflow (limited fields: display_name /
/// description / enabled / tags). Mirrors `skill::handlers::update_user_skill`.
/// Admin edits to system-scope items go through a future admin endpoint;
/// for now only the owner of a user-scope workflow may edit here.
pub async fn update_user_workflow(
    auth: RequirePermissions<(WorkflowsManage,)>,
    AxumPath(id): AxumPath<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateWorkflow>,
) -> ApiResult<Json<Workflow>> {
    let existing = repository::find_by_id(Repos.pool(), id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if existing.scope != "user" || existing.owner_user_id != Some(auth.user.id) {
        return Err::<_, (StatusCode, AppError)>(
            AppError::forbidden(
                "WORKFLOW_FORBIDDEN",
                "only the owner may edit a user-scope workflow",
            )
            .into(),
        );
    }
    let updated = repository::update(Repos.pool(), id, request).await?;
    crate::modules::workflow::events::emit_user_workflow(
        SyncAction::Update,
        id,
        auth.user.id,
        origin.0,
    );
    Ok((StatusCode::OK, Json(updated)))
}

pub fn update_user_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManage,)>(op)
        .id("Workflow.update")
        .tag("Workflows")
        .summary("Edit a user-owned workflow")
        .description("Update the editable metadata (display_name / description / enabled / tags) of a user-scope workflow the caller owns.")
        .response::<200, Json<Workflow>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Not the owner"))
        .response_with::<404, (), _>(|r| r.description("Workflow not found"))
}

// ============================================================
// Run / cancel
// ============================================================

pub async fn run_workflow(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    AxumPath(id): AxumPath<Uuid>,
    Json(req): Json<WorkflowRunRequest>,
) -> ApiResult<Json<WorkflowRunStartResponse>> {
    let pool = Repos.pool().clone();

    // Lookup workflow.
    let wf = repository::find_by_id(&pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    if wf.scope == "user" && wf.owner_user_id != Some(auth.user.id) {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_FORBIDDEN",
            "workflow owned by another user",
        )).into());
    }

    // Mocks are dev-only. Reject a /run that carries mocks against a
    // published (non-dev) workflow — prevents bypassing real execution
    // on production workflows. (Plan §1 + B4 audit MAJOR finding.)
    if !req.mocks.is_empty() && !wf.is_dev {
        return Err::<_, (StatusCode, AppError)>(
            (AppError::new(
                StatusCode::FORBIDDEN,
                "WORKFLOW_MOCKS_NOT_ALLOWED",
                "mocks are only honored for dev-imported workflows (is_dev=true)",
            ))
            .into(),
        );
    }

    // Shared spawn path (also used by workflow_mcp's tool-call handler).
    let run_id = crate::modules::workflow::runner::spawn_run(
        &pool,
        &wf,
        auth.user.id,
        req.conversation_id,
        req.inputs,
        req.mocks,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(WorkflowRunStartResponse {
            run_id,
            status: "pending".into(),
        }),
    ))
}

pub fn run_workflow_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.run")
        .tag("Workflows - Runs")
        .summary("Kick off a workflow run")
        .description("Synchronously returns {run_id}; progress streams via the per-run SSE endpoint.")
        .response::<202, Json<WorkflowRunStartResponse>>()
}

pub async fn cancel_run(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    AxumPath(run_id): AxumPath<Uuid>,
) -> ApiResult<Json<CancelAckResponse>> {
    let pool = Repos.pool();
    let row = repository::find_run(pool, run_id)
        .await?
        .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
    if row.user_id != auth.user.id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        )).into());
    }
    let prior = repository::cancel_cas(pool, run_id).await?;
    let _ = registry::cancel(run_id);
    let body = CancelAckResponse {
        status: prior.unwrap_or_else(|| "already_terminal".to_string()),
        run_id,
    };
    Ok((StatusCode::OK, Json(body)))
}

pub fn cancel_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.cancelRun")
        .tag("Workflows - Runs")
        .summary("Cancel an in-flight run")
        .response::<200, Json<CancelAckResponse>>()
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CancelAckResponse {
    pub status: String,
    pub run_id: Uuid,
}

pub async fn get_run(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath(run_id): AxumPath<Uuid>,
) -> ApiResult<Json<WorkflowRun>> {
    let row = repository::find_run(Repos.pool(), run_id)
        .await?
        .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
    if row.user_id != auth.user.id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        )).into());
    }
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.getRun")
        .tag("Workflows - Runs")
        .summary("Get a workflow run row")
        .response::<200, Json<WorkflowRun>>()
}

/// Best-effort removal of a workflow's extracted bundle dir on uninstall.
/// Mirrors the skill delete cleanup. A `NotFound` is treated as success
/// (the dir may already be gone); other errors are logged + swallowed so
/// the DB delete (the source of truth) still completes.
pub(crate) async fn remove_extracted_dir(extracted_path: &str) {
    match tokio::fs::remove_dir_all(extracted_path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(
                path = %extracted_path,
                error = %e,
                "workflow: failed to remove extracted bundle dir on uninstall"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn remove_extracted_dir_deletes_bundle_dir() {
        // Audit gap 1: workflow uninstall must rm the extracted bundle dir.
        let tmp = tempfile::tempdir().unwrap();
        let bundle = tmp.path().join("workflows/local.dev~x/0.0.0-dev");
        tokio::fs::create_dir_all(&bundle).await.unwrap();
        tokio::fs::write(bundle.join("workflow.yaml"), b"steps: []")
            .await
            .unwrap();
        assert!(bundle.exists());
        remove_extracted_dir(&bundle.display().to_string()).await;
        assert!(!bundle.exists(), "extracted dir should be removed");
    }

    #[tokio::test]
    async fn remove_extracted_dir_ignores_missing() {
        // A second uninstall / already-cleaned dir is not an error.
        let tmp = tempfile::tempdir().unwrap();
        let gone = tmp.path().join("never-existed");
        // Must not panic / must complete cleanly.
        remove_extracted_dir(&gone.display().to_string()).await;
        assert!(!gone.exists());
    }
}


