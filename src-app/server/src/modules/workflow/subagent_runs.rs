//! ITEM-8 — typed REST to VIEW a chat turn's fan-out CHILD sub-agent transcripts.
//!
//!   - `GET /api/subagent-runs?parent_message_id={uuid}` — the children of one
//!     parent assistant message (compact: id / label / status / created_at).
//!   - `GET /api/subagent-runs/{child_id}`               — one child's FULL detail
//!     including its `activity[]` transcript.
//!
//! Both are:
//!   - **owner-scoped** by `user_id` — a foreign / missing child yields **404**,
//!     never leaking another user's run (DEC-14 / CODING_GUIDELINES §1);
//!   - **gated `WorkflowsRead`** — the EXISTING permission held by the Users group
//!     (child rows are `workflow_runs` rows); NO new permission is introduced.
//!
//! The DETAIL handler REUSES `get_background_run_detail` — a `subagent` child is a
//! background-kind run (`job_kind <> 'workflow'`), so that getter already
//! owner-scopes, 404s on a foreign/missing/workflow id, and returns the `activity`
//! projection. Only the parent-scoped LIST is new.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::workflow::permissions::WorkflowsRead;
use crate::modules::workflow::repository as wf_repo;
use crate::modules::workflow::types::{BackgroundRunDetail, SubAgentRunListResponse};

/// Query params for `GET /api/subagent-runs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSubAgentRunsQuery {
    /// The parent assistant `message_id` whose fan-out children to list.
    pub parent_message_id: Uuid,
}

#[debug_handler]
pub async fn list_subagent_runs(
    auth: RequirePermissions<(WorkflowsRead,)>,
    Query(q): Query<ListSubAgentRunsQuery>,
) -> ApiResult<Json<SubAgentRunListResponse>> {
    // Owner-scoped: children of a parent message the acting user does not own come
    // back empty (never another user's rows).
    let children =
        wf_repo::list_subagent_children(Repos.pool(), q.parent_message_id, auth.user.id).await?;
    Ok((StatusCode::OK, Json(SubAgentRunListResponse { children })))
}

pub fn list_subagent_runs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("SubAgentRuns.list")
        .tag("workflow")
        .summary("List a chat turn's fan-out sub-agent children")
        .description(
            "Owner-scoped list of the fan-out CHILD sub-agent runs spawned under one \
             parent assistant message (`parent_message_id`), spawn-ordered, compact \
             (id / label / status / created_at). The full per-child transcript is \
             fetched via `GET /api/subagent-runs/{id}`. A parent message the acting \
             user does not own yields an empty list. Hard-bounded by the fan-out cap.",
        )
        .response::<200, Json<SubAgentRunListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

#[debug_handler]
pub async fn get_subagent_run(
    auth: RequirePermissions<(WorkflowsRead,)>,
    Path(child_id): Path<Uuid>,
) -> ApiResult<Json<BackgroundRunDetail>> {
    // A child is a background-kind run, so the background-detail getter owner-scopes,
    // 404s a foreign/missing/workflow id, and returns the `activity[]` transcript.
    let detail = wf_repo::get_background_run_detail(Repos.pool(), child_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Sub-agent run"))?;
    Ok((StatusCode::OK, Json(detail)))
}

pub fn get_subagent_run_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsRead,)>(op)
        .id("SubAgentRuns.get")
        .tag("workflow")
        .summary("Get one fan-out sub-agent child's full transcript")
        .description(
            "Owner-scoped full detail for one fan-out CHILD sub-agent run, including \
             its `activity[]` transcript (thinking / tool calls & results / messages), \
             rendered by the workflow `AgentActivityTimeline`. A foreign / missing id \
             → 404 (never leaked).",
        )
        .response::<200, Json<BackgroundRunDetail>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Sub-agent run not found / not owned"))
}
