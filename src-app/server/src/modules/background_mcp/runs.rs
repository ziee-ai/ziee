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
    /// Scope to ONE conversation's background runs — used by the in-chat "Tasks"
    /// panel + the end-of-conversation affordance.
    ///
    /// **Disjoint**, not additive: omitting it returns ONLY the conversation-LESS
    /// runs (detached work, e.g. a scheduled task's), never every run. A run
    /// therefore appears in exactly one surface.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
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
        // Bound as `status = $2` / `job_kind = $3` with no other validation,
        // so both need the shared NUL guard (a NUL here 500'd).
        //
        // `guard_raw`, NOT `normalize_text_filter`: this call site binds the
        // RAW value, so trimming or mapping blank to None would silently widen
        // `?status=` from "match the empty string" (0 rows) to "no filter"
        // (every run the caller owns). Guard the value; do not rewrite it.
        crate::common::text_guard::guard_raw(params.status.as_deref(), "status")?,
        crate::common::text_guard::guard_raw(params.kind.as_deref(), "kind")?,
        params.conversation_id,
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
             via `collect_result`.\n\n\
             `conversation_id` is a DISJOINT scope: omit it and you get ONLY the \
             conversation-less runs (detached work such as a scheduled task's); pass one \
             and you get ONLY that conversation's runs. A background run is therefore \
             surfaced in exactly one place — its conversation's in-chat Tasks panel, or \
             the scheduler's run history — never both.",
        )
        .response::<200, Json<BackgroundRunListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<400, (), _>(|res| {
            res.description("Invalid query parameter (e.g. a NUL byte in a free-text filter)")
        })
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

    // Owner-scope + background-only: a foreign / missing run — or a classic
    // `job_kind='workflow'` run (mutating a workflow run through the background
    // endpoint is out of bounds; `/api/workflows/*` owns those) — → 404 (never
    // leak — DEC-36 §1). Mirrors `get_background_run_detail`'s filter.
    let run = wf_repo::find_background_run_for_owner(Repos.pool(), run_id, user_id)
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
        .response_with::<404, (), _>(|r| r.description("Run not found / not owned"))
        .response_with::<409, (), _>(|r| r.description("Run already finished"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;
    use axum::http::Uri;

    fn parse(qs: &str) -> Result<ListBackgroundRunsQuery, String> {
        let uri: Uri = format!("/background/runs?{qs}").parse().unwrap();
        Query::<ListBackgroundRunsQuery>::try_from_uri(&uri)
            .map(|Query(q)| q)
            .map_err(|e| e.to_string())
    }

    /// TEST-1 — the disjoint `conversation_id` scope must survive deserialization
    /// exactly as sent. A filter that silently vanishes here would WIDEN the
    /// scope (the panel would fall back to the conversation-less listing), which
    /// is precisely the failure this param exists to prevent.
    #[test]
    fn conversation_id_is_parsed_when_present() {
        let id = Uuid::new_v4();
        let q = parse(&format!("conversation_id={id}")).expect("valid query");
        assert_eq!(q.conversation_id, Some(id));
        // The other filters keep their documented defaults.
        assert_eq!(q.page, 1);
        assert_eq!(q.per_page, 50);
        assert!(q.status.is_none());
        assert!(q.kind.is_none());
    }

    #[test]
    fn conversation_id_is_none_when_absent() {
        let q = parse("page=2&per_page=20").expect("valid query");
        assert!(
            q.conversation_id.is_none(),
            "absent conversation_id must stay None — the repository reads None as \
             'conversation-less runs only', so any other value silently rescopes"
        );
        assert_eq!(q.page, 2);
        assert_eq!(q.per_page, 20);
    }

    #[test]
    fn conversation_id_composes_with_the_other_filters() {
        let id = Uuid::new_v4();
        let q = parse(&format!("status=running&kind=subagent&conversation_id={id}"))
            .expect("valid query");
        assert_eq!(q.conversation_id, Some(id));
        assert_eq!(q.status.as_deref(), Some("running"));
        assert_eq!(q.kind.as_deref(), Some("subagent"));
    }

    #[test]
    fn malformed_conversation_id_is_rejected_not_dropped() {
        let err = parse("conversation_id=not-a-uuid")
            .expect_err("a malformed uuid must be a 4xx, never a silently-dropped filter");
        // Name the FIELD. A bare `!err.is_empty()` fallback would make this
        // assertion true for any rejection at all, so it could not distinguish
        // "rejected because conversation_id is malformed" from any other 4xx.
        assert!(
            err.to_lowercase().contains("conversation_id"),
            "the rejection must name the offending field, got: {err}"
        );
    }
}

/// Cancel every still-in-flight background run bound to `conversation_id` before
/// that conversation is DELETED.
///
/// **Why this exists.** `workflow_runs_conversation_id_fkey` is
/// `ON DELETE SET NULL`, so deleting a conversation would otherwise DETACH its
/// background runs: the rows survive with `conversation_id = NULL` and — worse —
/// the spawned tasks keep executing headlessly, while the only surface that could
/// view, steer, or cancel them (that conversation's in-chat "Tasks" panel) is gone.
/// There is deliberately no global background page, so a detached run would be
/// unreachable forever. Closing the hole AT THE SOURCE keeps the design intact:
/// nothing survives detached because nothing survives non-terminal.
///
/// **What it does per run**, reusing the single-run cancel path verbatim (see
/// `cancel_background_run`) rather than reinventing it:
///   1. `cancel_cas` — the status-guarded terminal write (`pending`/`running`/
///      `waiting`/`resumable` → `cancelled`). First terminal writer wins, so this
///      is idempotent against a run finishing concurrently.
///   2. `registry::cancel` — wakes the run's in-memory `RunHandle`, which the
///      sub-agent driver bridges into the agent-core `CancelToken`, so the DETACHED
///      TASK actually stops at its next await point instead of running on
///      headlessly. This is the half that matters most here: cancelling only the
///      row would leave the work executing.
///
/// An ALREADY-TERMINAL run is left completely alone — the query excludes it and the
/// CAS would refuse it anyway. Its row keeps its result and simply becomes
/// conversation-less, which is fine: a terminal run has nothing to steer or stop.
///
/// Owner-scoped: `user_id` is threaded into the query, so a caller who does not own
/// the conversation cancels nothing.
///
/// Best-effort by design — a DB error here is logged and the delete proceeds. The
/// alternative (aborting the user's delete because a cancel failed) is worse: the
/// startup orphan sweep already reconciles rows left non-terminal by a crash.
/// Returns the ids it cancelled, for the caller's sync fan-out + tests.
pub async fn cancel_conversation_background_runs(
    conversation_id: Uuid,
    user_id: Uuid,
) -> Vec<Uuid> {
    let ids = match wf_repo::list_cancellable_background_runs_for_conversation(
        Repos.pool(),
        conversation_id,
        user_id,
    )
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(
                %conversation_id,
                error = %e,
                "background: could not list in-flight runs for a conversation being deleted; \
                 they may be left non-terminal for the startup orphan sweep"
            );
            return Vec::new();
        }
    };
    if ids.is_empty() {
        return ids;
    }

    let mut cancelled = Vec::with_capacity(ids.len());
    for run_id in ids {
        // DB authority first (survives a crashed/absent runner), then the
        // in-memory signal so the detached task stops executing.
        match wf_repo::cancel_cas(Repos.pool(), run_id).await {
            Ok(Some(_)) => {
                let _ = registry::cancel(run_id);
                cancelled.push(run_id);
            }
            // Raced to terminal between the list and the CAS — nothing to do.
            Ok(None) => {}
            Err(e) => tracing::warn!(
                %run_id, %conversation_id, error = %e,
                "background: cancel-on-conversation-delete failed for this run"
            ),
        }
    }
    if !cancelled.is_empty() {
        tracing::info!(
            %conversation_id,
            count = cancelled.len(),
            "background: cancelled in-flight runs for a deleted conversation"
        );
    }
    cancelled
}
