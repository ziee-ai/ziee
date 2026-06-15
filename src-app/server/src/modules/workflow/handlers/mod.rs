//! Workflow REST handlers (user + admin split; B4).
//!
//! User: list / get / delete + install-from-hub re-bind + RUN + CANCEL.
//! Admin (`/system/*`): list / delete + group assignment (TODO B6).
//! `/import`, `/validate`, `/dry-run`, `/test` are stubbed —
//! they need additional plumbing (multipart upload, mock cost
//! estimation) and land in B6.

#![allow(dead_code)]

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
use crate::modules::workflow::models::{Workflow, WorkflowRun};
use crate::modules::workflow::permissions::{
    WorkflowsExecute, WorkflowsManage, WorkflowsRead,
};
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{
    WorkflowListResponse, WorkflowRunRequest, WorkflowRunStartResponse,
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

// Stub for /validate, /import, /dry-run, /test — Phase B6.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DeferredResponse {
    pub status: String,
    pub message: String,
}

pub async fn validate_stub(
    _auth: RequirePermissions<(WorkflowsRead,)>,
) -> ApiResult<Json<DeferredResponse>> {
    Ok((
        StatusCode::ACCEPTED,
        Json(DeferredResponse {
            status: "deferred".into(),
            message: "POST /api/workflows/validate lands in Phase B6".into(),
        }),
    ))
}

pub fn validate_stub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.validateStub")
        .tag("Workflows")
        .summary("[Phase B6] Validate a workflow.yaml without installing")
        .response::<202, Json<DeferredResponse>>()
}

