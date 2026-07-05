//! Workflow REST handlers (user + admin split; B4 + B6).
//!
//! User: list / get / delete + install-from-hub re-bind + RUN + CANCEL.
//! Admin (`/system/*`): list / delete + group assignment.
//! `/validate`, `/import`, `/dry-run`, `/test` (B6) live in `dev.rs`.


pub mod dev;
pub mod system;

use aide::transform::TransformOperation;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{ApiResult, AppError, DEFAULT_PAGE_SIZE, PAGINATION_MAX_PER_PAGE};
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

/// Optional pagination for the workflow listing. Defaults bound an
/// un-paginated caller to the first `DEFAULT_PAGE_SIZE` workflows instead of
/// returning an unbounded set.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkflowListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_user_workflows(
    auth: RequirePermissions<(WorkflowsRead,)>,
    Query(q): Query<WorkflowListQuery>,
) -> ApiResult<Json<WorkflowListResponse>> {
    let limit = q
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE as i64)
        .clamp(1, PAGINATION_MAX_PER_PAGE as i64);
    let offset = q.offset.unwrap_or(0).max(0);
    let workflows =
        repository::list_for_user(Repos.pool(), auth.user.id, limit, offset).await?;
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
    // H2: gate on full access (user-owned OR group-accessible system).
    // A group-restricted system workflow the caller is NOT a member of
    // must 404, not leak via GET.
    if !repository::user_can_access(Repos.pool(), auth.user.id, id).await? {
        return Err::<_, (StatusCode, AppError)>(AppError::not_found("Workflow").into());
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

/// Best-effort cleanup of a single run's on-disk artifacts (run-created file
/// blobs + the staged run dir). Shared by `delete_run` semantics and the
/// workflow-delete cascade. A run that belongs to a conversation keeps its
/// files (they belong to the chat context).
async fn cleanup_run_artifacts(user_id: Uuid, run_id: Uuid, conversation_id: Option<Uuid>) {
    if conversation_id.is_none() {
        let storage = crate::modules::file::storage::manager::get_file_storage();
        if let Ok(fids) = Repos.file.list_ids_by_workflow_run(run_id).await {
            for fid in fids {
                match Repos.file.delete(fid, user_id).await {
                    Ok(blob_ids) => {
                        for blob_id in blob_ids {
                            let _ = storage.delete_all(user_id, blob_id).await;
                        }
                    }
                    Err(e) => tracing::warn!("workflow: delete run artifact {fid} failed: {e}"),
                }
            }
        }
    }
    let root = crate::modules::workflow::runner::workflow_workspace_root();
    let conv_or_run = conversation_id.unwrap_or(run_id);
    let run_dir = root
        .join(conv_or_run.to_string())
        .join("workflow")
        .join(run_id.to_string());
    let _ = tokio::fs::remove_dir_all(&run_dir).await;
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
    // Clean up each run's on-disk artifacts BEFORE the workflow_runs rows
    // cascade away — `files.workflow_run_id` is `ON DELETE SET NULL`, so
    // run-created blobs + staged dirs would otherwise be orphaned forever.
    if let Ok(runs) = repository::list_run_refs_for_workflow(Repos.pool(), id).await {
        for (run_id, conv_id) in runs {
            cleanup_run_artifacts(auth.user.id, run_id, conv_id).await;
        }
    }
    // L4: delete the DB row (source of truth) FIRST, then best-effort rm
    // the extracted bundle dir. Mirrors `skill::handlers::delete_user_skill`
    // — if the DB delete fails we bail with the row + dir both intact,
    // rather than leaving an orphan row pointing at an already-removed dir.
    repository::delete(Repos.pool(), id).await?;
    remove_extracted_dir(&wf.extracted_path).await;
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
    // H2: a group-restricted system workflow must be unrunnable to a
    // non-member (the skill side already enforces this via user_can_read).
    if !repository::user_can_access(&pool, auth.user.id, id).await? {
        return Err::<_, (StatusCode, AppError)>(AppError::not_found("Workflow").into());
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
        crate::modules::workflow::runner::SpawnRunOpts {
            model_id: req.model_id,
            invocation_source: "manual",
            persist_artifacts: true,
            force_log_capture: req.capture_logs,
        },
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
    origin: SyncOrigin,
    AxumPath(run_id): AxumPath<Uuid>,
) -> ApiResult<Json<RunActionAck>> {
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
    // A live run's runner task emits WorkflowRun when it observes the
    // cancellation, but a durably-suspended (waiting) or queued (pending) run
    // has no live task — emit directly so other devices' run lists update.
    if prior.is_some() {
        crate::modules::workflow::events::emit_workflow_run(
            SyncAction::Update,
            run_id,
            auth.user.id,
            origin.0,
        );
    }
    let body = RunActionAck {
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
        .response::<200, Json<RunActionAck>>()
}

/// Acknowledgement for an in-flight-run action (cancel / set-timeout). `status`
/// is the action-specific outcome string ("cancelled" / "updated" /
/// "already_terminal"); shared intentionally by both endpoints.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RunActionAck {
    pub status: String,
    pub run_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTimeoutRequest {
    /// New wall-clock cap in seconds; `0` = unbounded (no timeout). Named to
    /// match the `timeout_secs` used across the rest of the timeout surface.
    pub timeout_secs: u64,
}

/// Change an in-flight run's wall-clock timeout LIVE (extend / shorten / lift).
/// The runner's deadline watcher honors the new value within its recheck
/// interval. The per-run token + output-byte caps stay as resource backstops.
pub async fn set_run_timeout(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    origin: SyncOrigin,
    AxumPath(run_id): AxumPath<Uuid>,
    Json(req): Json<SetTimeoutRequest>,
) -> ApiResult<Json<RunActionAck>> {
    let pool = Repos.pool();
    let row = repository::find_run(pool, run_id)
        .await?
        .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
    if row.user_id != auth.user.id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        ))
        .into());
    }
    // Clamp to the engine ceiling (0 = unbounded stays 0) — guards the
    // `deadline_watcher` Instant arithmetic against a pathological value.
    let secs = if req.timeout_secs == 0 {
        0
    } else {
        req.timeout_secs
            .min(crate::modules::workflow::runner::MAX_RUN_TIMEOUT_SECS)
    };
    let applied = registry::set_timeout(run_id, secs);
    // Notify other devices so their run views reflect the new timeout.
    if applied {
        crate::modules::workflow::events::emit_workflow_run(
            SyncAction::Update,
            run_id,
            auth.user.id,
            origin.0,
        );
    }
    let body = RunActionAck {
        status: if applied { "updated".into() } else { "already_terminal".into() },
        run_id,
    };
    Ok((StatusCode::OK, Json(body)))
}

pub fn set_run_timeout_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.setRunTimeout")
        .tag("Workflows - Runs")
        .summary("Change an in-flight run's wall-clock timeout (secs; 0 = unbounded)")
        .response::<200, Json<RunActionAck>>()
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

// ============================================================
// A4 — run history list
// ============================================================

pub async fn list_workflow_runs(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath(id): AxumPath<Uuid>,
) -> ApiResult<Json<crate::modules::workflow::types::WorkflowRunListResponse>> {
    // Owner-scoped: only the caller's own runs of this workflow. A system
    // workflow is visible to many users, but its runs are per-user.
    let runs = repository::list_runs_for_workflow(Repos.pool(), id, auth.user.id, 200).await?;
    Ok((
        StatusCode::OK,
        Json(crate::modules::workflow::types::WorkflowRunListResponse { runs }),
    ))
}

pub fn list_workflow_runs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.listRuns")
        .tag("Workflows - Runs")
        .summary("List a workflow's runs (owner-scoped history)")
        .response::<200, Json<crate::modules::workflow::types::WorkflowRunListResponse>>()
}

// ============================================================
// A5 — delete a run (+ conditional artifact cascade)
// ============================================================

pub async fn delete_run(
    auth: RequirePermissions<(WorkflowsExecute,)>,
    origin: SyncOrigin,
    AxumPath(run_id): AxumPath<Uuid>,
) -> ApiResult<()> {
    let pool = Repos.pool();
    let row = repository::find_run(pool, run_id)
        .await?
        .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
    if row.user_id != auth.user.id {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::FORBIDDEN,
            "WORKFLOW_RUN_FORBIDDEN",
            "workflow run is owned by another user",
        ))
        .into());
    }
    // Only terminal runs are deletable — cancel an in-flight run first.
    let terminal = crate::modules::workflow::models::WorkflowRunStatus::from_db_str(&row.status)
        .is_some_and(|s| s.is_terminal());
    if !terminal {
        return Err::<_, (StatusCode, AppError)>((AppError::new(
            StatusCode::CONFLICT,
            "WORKFLOW_RUN_NOT_TERMINAL",
            "cancel the run before deleting it",
        ))
        .into());
    }
    // Cascade artifacts ONLY for a run with no conversation. The run owns (and
    // deletes) the files IT created (created_by="workflow" + workflow_run_id);
    // a conversation-run's files belong to the chat context and are kept.
    if row.conversation_id.is_none() {
        let storage = crate::modules::file::storage::manager::get_file_storage();
        for fid in Repos.file.list_ids_by_workflow_run(run_id).await? {
            match Repos.file.delete(fid, auth.user.id).await {
                Ok(blob_ids) => {
                    for blob_id in blob_ids {
                        let _ = storage.delete_all(auth.user.id, blob_id).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("workflow: delete run artifact {fid} failed: {e}")
                }
            }
        }
    }
    // Remove the staged dir (outputs / artifacts / logs on disk).
    let root = crate::modules::workflow::runner::workflow_workspace_root();
    let conv_or_run = row.conversation_id.unwrap_or(row.id);
    let run_dir = root
        .join(conv_or_run.to_string())
        .join("workflow")
        .join(row.id.to_string());
    let _ = tokio::fs::remove_dir_all(&run_dir).await;

    repository::delete_run_row(pool, run_id).await?;
    crate::modules::workflow::events::emit_workflow_run(
        SyncAction::Delete,
        run_id,
        auth.user.id,
        origin.0,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsExecute,)>(op)
        .id("Workflow.deleteRun")
        .tag("Workflows - Runs")
        .summary("Delete a terminal run (+ its artifacts when not tied to a conversation)")
        .response::<204, ()>()
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


