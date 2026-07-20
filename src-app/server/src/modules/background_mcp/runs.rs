//! ITEM-8 / ITEM-10 — typed REST to VIEW + MANAGE the acting user's background
//! runs:
//!   - `GET  /api/background/runs`                 — list (paginated, filterable)
//!   - `GET  /api/background/runs/{run_id}`        — one run's full detail (incl. result)
//!   - `POST /api/background/runs/{run_id}/cancel` — cancel a running run
//!
//! Both are:
//!   - **owner-scoped** — resolved via the `workflow_runs` background-run backbone
//!     (`job_kind <> 'workflow'`); a foreign / missing run yields **404**, never
//!     leaking another user's run (DEC-16 / DEC-36 / CODING_GUIDELINES §1);
//!   - **gated `background::use`** — the SAME permission the backbone's
//!     model-facing reads (`check_status` / `collect_result`) + the steering-note
//!     REST use.
//!
//! The list is a COMPACT projection (no heavy JSONB blobs, no `final_output_json`
//! — that's read via the single-run detail getter, or the `collect_result` MCP
//! tool); `has_result` flags whether a result is ready. The detail getter is
//! owner-scoped + background-only (a classic workflow run → 404) and adds the
//! `final_output_json` result body on top of the summary fields. Cancel reuses
//! the EXISTING run-cancel mechanism — the status-guarded
//! `repository::cancel_cas` (DB authority) + `registry::cancel` (the in-memory
//! signal the detached sub-agent task observes via its `RunHandle` → the
//! agent-core `CancelToken`). No new cancel primitive is introduced.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncOrigin};
use crate::modules::workflow::events::emit_workflow_run;
use crate::modules::workflow::models::WorkflowRunStatus;
use crate::modules::workflow::registry;
use crate::modules::workflow::repository as wf_repo;
use crate::modules::workflow::types::{BackgroundRunDetail, BackgroundRunListResponse};

use super::permissions::BackgroundUse;

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

/// Query params for `GET /api/background/runs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBackgroundRunsQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    /// Page size; clamped to `1..=500` server-side (default 50).
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter to a single run status (`pending` / `running` / `waiting` /
    /// `resumable` / `completed` / `failed` / `cancelled`).
    #[serde(default)]
    pub status: Option<String>,
    /// Filter to a single background job kind (`subagent` / `sandbox_exec`).
    #[serde(default)]
    pub kind: Option<String>,
}

#[debug_handler]
pub async fn list_background_runs(
    auth: RequirePermissions<(BackgroundUse,)>,
    Query(params): Query<ListBackgroundRunsQuery>,
) -> ApiResult<Json<BackgroundRunListResponse>> {
    let page = params.page.max(1);
    let per_page = params.per_page.clamp(1, 500);
    let (runs, total) = wf_repo::list_background_runs_for_user(
        Repos.pool(),
        auth.user.id,
        page,
        per_page,
        params.status.as_deref(),
        params.kind.as_deref(),
    )
    .await?;
    let total_pages = if per_page > 0 {
        (total + per_page - 1) / per_page
    } else {
        0
    };
    Ok((
        StatusCode::OK,
        Json(BackgroundRunListResponse {
            runs,
            total,
            page,
            per_page,
            total_pages,
        }),
    ))
}

pub fn list_background_runs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BackgroundUse,)>(op)
        .id("Background.listRuns")
        .tag("background")
        .summary("List the acting user's background runs")
        .description(
            "Owner-scoped, newest-first, paginated list of the caller's background runs \
             (detached sub-agent / sandbox-exec runs — never classic workflow runs). \
             Optional `status` / `kind` filters; `page`/`per_page` clamped (default 50, \
             cap 500). Compact summaries only — the full result is fetched separately \
             via `collect_result`.",
        )
        .response::<200, Json<BackgroundRunListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Missing background::use"))
}

#[debug_handler]
pub async fn get_background_run(
    auth: RequirePermissions<(BackgroundUse,)>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<BackgroundRunDetail>> {
    // Owner-scope + background-only: a foreign / missing / classic-workflow-kind
    // run → 404 (never leak; workflow runs are served by their own endpoint —
    // DEC-36 §1).
    let detail = wf_repo::get_background_run_detail(Repos.pool(), run_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Background run"))?;
    Ok((StatusCode::OK, Json(detail)))
}

pub fn get_background_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BackgroundUse,)>(op)
        .id("Background.getRun")
        .tag("background")
        .summary("Get one background run incl. its full result")
        .description(
            "Owner-scoped detail for a single background run (a detached sub-agent / \
             sandbox-exec run — never a classic workflow run), including the full \
             `final_output_json` result body plus status, error, timings, kind, and \
             tokens. This is the getter the FE uses to render a COMPLETED run's result — \
             the list endpoint returns only compact summaries with `has_result`. A \
             foreign / missing / classic-workflow-kind run → 404 (never leaked; classic \
             workflow runs are served by `GET /api/workflows/runs/{id}`).",
        )
        .response::<200, Json<BackgroundRunDetail>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Missing background::use"))
        .response_with::<404, (), _>(|r| r.description("Run not found / not owned"))
}

/// Cancel-run acknowledgement.
#[derive(Debug, Serialize, JsonSchema)]
pub struct BackgroundRunCancelAck {
    /// `"cancelled"` when the run was flipped; `"already_terminal"` on the benign
    /// race where the run reached terminal between the ownership check and the CAS.
    pub status: String,
    pub run_id: Uuid,
}

#[debug_handler]
pub async fn cancel_background_run(
    auth: RequirePermissions<(BackgroundUse,)>,
    Path(run_id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<Json<BackgroundRunCancelAck>> {
    let user_id = auth.user.id;

    // Owner-scope: a foreign / missing run → 404 (never leak — DEC-36 §1).
    let run = wf_repo::find_run_for_owner(Repos.pool(), run_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Background run"))?;

    // A terminal run has nothing to cancel → 409.
    if WorkflowRunStatus::from_db_str(&run.status)
        .map(|s| s.is_terminal())
        .unwrap_or(false)
    {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "RUN_ALREADY_TERMINAL",
            "background run has already finished; it cannot be cancelled",
        )
        .into());
    }

    // Flip the DB row (status-guarded CAS — the FIRST terminal writer wins) AND
    // fire the in-memory cancel so the DETACHED sub-agent task stops at its next
    // await point. `registry::cancel` wakes the run's `RunHandle`, which the
    // sub-agent driver (`tools::drive_subagent_turn`) bridges into the agent-core
    // `CancelToken` → the loop `Halt`s → the driver reports
    // `BackgroundOutcome::Cancelled` → `spawn_background_run` re-asserts the
    // terminal `cancelled` write (idempotent with this CAS). `cancel_cas` is the
    // authority for a run whose in-memory handle is already gone (crashed runner /
    // a cold `waiting` gate with no resident task).
    let prior = wf_repo::cancel_cas(Repos.pool(), run_id).await?;
    let _ = registry::cancel(run_id);

    // Owner-scoped notify-and-refetch. A live run's task ALSO emits `WorkflowRun`
    // on its Cancelled transition, but a cold (`waiting`) run has no task — emit
    // here so every device's list updates immediately (mirrors workflow
    // `cancel_run`; reuses `SyncEntity::WorkflowRun` per DEC-13/32).
    if prior.is_some() {
        emit_workflow_run(SyncAction::Update, run_id, user_id, origin.0);
    }

    Ok((
        StatusCode::OK,
        Json(BackgroundRunCancelAck {
            status: prior
                .map(|_| "cancelled".to_string())
                .unwrap_or_else(|| "already_terminal".to_string()),
            run_id,
        }),
    ))
}

pub fn cancel_background_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BackgroundUse,)>(op)
        .id("Background.cancelRun")
        .tag("background")
        .summary("Cancel a running background run")
        .description(
            "Marks a non-terminal background run cancelled and signals the detached task \
             to stop (reusing the run-cancel CAS + the in-memory `RunHandle` cancel). \
             Owner-scoped (a foreign/missing run → 404); an already-finished run → 409.",
        )
        .response::<200, Json<BackgroundRunCancelAck>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Missing background::use"))
        .response_with::<404, (), _>(|r| r.description("Run not found / not owned"))
        .response_with::<409, (), _>(|r| r.description("Run already finished"))
}
