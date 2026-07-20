//! ITEM-25 — typed REST for STEERING a running background run (Group F).
//!
//! `POST /api/background/runs/{run_id}/notes` enqueues a durable steering note the
//! detached sub-agent picks up on its next turn; `GET .../notes` lists the run's
//! pending (not-yet-consumed) notes. Both are:
//!   - **owner-scoped + background-only** — resolved via
//!     `workflow::repository::find_background_run_for_owner`, so a foreign /
//!     missing run — or a classic `job_kind='workflow'` run — yields **404**,
//!     never leaking another user's run and never steering a workflow run through
//!     the background surface (DEC-36 / CODING_GUIDELINES §1);
//!   - **gated `background::use`** — the SAME permission the backbone's
//!     model-facing reads (`check_status` / `collect_result`) use.
//!
//! The durable queue lives in the workflow backbone (`background_run_notes` table
//! + `workflow::repository::{enqueue,list_pending}_run_notes`). The detached
//! agent-core loop CONSUMES pending notes at its next iteration boundary via
//! `workflow::repository::consume_pending_run_notes` — a FLAGGED FOLLOW-UP (an
//! agent-core port + a `build_detached_agent_core` impl; see that fn's doc).

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Path,
    http::StatusCode,
};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncOrigin};
use crate::modules::workflow::events::emit_workflow_run;
use crate::modules::workflow::models::{CreateRunNote, RunNote, WorkflowRunStatus};
use crate::modules::workflow::repository as wf_repo;

use super::permissions::BackgroundUse;

/// Max length of one steering note (chars). A note is a short nudge, not a
/// document; bound it so the durable queue + the transcript injection stay cheap.
const MAX_NOTE_CHARS: usize = 4000;

/// Owner-scope a BACKGROUND run for the acting user, 404 on foreign/missing —
/// and on a classic `job_kind='workflow'` run (never leak, and never steer a
/// workflow run through the background surface; DEC-36 §1). Mirrors the
/// list/detail endpoints' `job_kind <> 'workflow'` boundary.
async fn owned_run_status(run_id: Uuid, user_id: Uuid) -> Result<String, AppError> {
    let run = wf_repo::find_background_run_for_owner(Repos.pool(), run_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Background run"))?;
    Ok(run.status)
}

#[debug_handler]
pub async fn post_run_note(
    auth: RequirePermissions<(BackgroundUse,)>,
    Path(run_id): Path<Uuid>,
    origin: SyncOrigin,
    Json(req): Json<CreateRunNote>,
) -> ApiResult<Json<RunNote>> {
    let user_id = auth.user.id;
    let status = owned_run_status(run_id, user_id).await?;

    // A terminal run has no turn loop left to read the note → 409.
    if WorkflowRunStatus::from_db_str(&status)
        .map(|s| s.is_terminal())
        .unwrap_or(false)
    {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "RUN_FINISHED",
            "background run has already finished; it cannot be steered",
        )
        .into());
    }

    let note = req.note.trim();
    if note.is_empty() {
        return Err(
            AppError::bad_request("EMPTY_NOTE", "steering note must not be empty").into(),
        );
    }
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::bad_request(
            "NOTE_TOO_LONG",
            format!("steering note exceeds {MAX_NOTE_CHARS} characters"),
        )
        .into());
    }

    let row = wf_repo::enqueue_run_note(Repos.pool(), run_id, note).await?;

    // Owner-scoped notify-and-refetch: the FE run view refreshes (surfacing the
    // pending steering note). Reuses SyncEntity::WorkflowRun (DEC-13/31 — no new
    // entity for the steer channel).
    emit_workflow_run(SyncAction::Update, run_id, user_id, origin.0);
    Ok((StatusCode::CREATED, Json(row)))
}

pub fn post_run_note_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BackgroundUse,)>(op)
        .id("Background.postRunNote")
        .tag("background")
        .summary("Queue a steering note to a running background run")
        .description(
            "Enqueues a durable steering note the detached sub-agent picks up on its next turn. \
             Owner-scoped (a foreign/missing run → 404); a finished run → 409. The pending queue \
             is bounded (newest 8 kept).",
        )
        .response::<201, Json<RunNote>>()
        .response_with::<400, (), _>(|r| r.description("Empty or over-length note"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Run not found / not owned"))
        .response_with::<409, (), _>(|r| r.description("Run already finished"))
}

#[debug_handler]
pub async fn list_run_notes(
    auth: RequirePermissions<(BackgroundUse,)>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<Vec<RunNote>>> {
    let user_id = auth.user.id;
    // Owner-scope first (404 on foreign/missing), then read pending notes.
    owned_run_status(run_id, user_id).await?;
    let notes = wf_repo::list_pending_run_notes(Repos.pool(), run_id).await?;
    Ok((StatusCode::OK, Json(notes)))
}

pub fn list_run_notes_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BackgroundUse,)>(op)
        .id("Background.listRunNotes")
        .tag("background")
        .summary("List a running background run's pending steering notes")
        .description(
            "Returns the run's not-yet-consumed steering notes, oldest-first. \
             Owner-scoped (a foreign/missing run → 404).",
        )
        .response::<200, Json<Vec<RunNote>>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Run not found / not owned"))
}
