//! Per-run SSE endpoint (plan §4.4).
//!
//! `GET /api/workflow-runs/{run_id}/events` streams every
//! `SSEWorkflowRunEvent` the runner emits. First frame is a `Snapshot`
//! built from the current `workflow_runs` row (metadata blobs + status
//! + pending_elicitation_json), so a freshly-mounted FE skips the
//! separate `GET /api/workflow-runs/{id}` call.

#![allow(dead_code)]

use std::convert::Infallible;

use aide::transform::TransformOperation;
use async_stream::stream;
use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::workflow::events::{
    SSEConnectedData, SSESnapshotData, SSEWorkflowRunEvent,
};
use crate::modules::workflow::permissions::WorkflowsRead;
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;

pub async fn subscribe(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath(run_id): AxumPath<Uuid>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Auth: caller must own the run.
    let row = repository::find_run(Repos.pool(), run_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                AppError::not_found("WorkflowRun"),
            )
        })?;
    if row.user_id != auth.user.id {
        return Err((
            StatusCode::FORBIDDEN,
            AppError::new(
                StatusCode::FORBIDDEN,
                "WORKFLOW_RUN_FORBIDDEN",
                "workflow run is owned by another user",
            ),
        ));
    }

    // Snapshot first.
    let snapshot = SSEWorkflowRunEvent::Snapshot(SSESnapshotData {
        run_id,
        status: row.status.clone(),
        current_step: row.current_step.clone(),
        total_tokens: row.total_tokens as u64,
        step_outputs_json: row.step_outputs_json.clone(),
        step_item_progress_json: row.step_item_progress_json.clone(),
        step_logs_json: row.step_logs_json.clone(),
        step_artifacts_json: row.step_artifacts_json.clone(),
        pending_elicitation_json: row.pending_elicitation_json.clone(),
        final_output_json: row.final_output_json.clone(),
    });

    let terminal = matches!(row.status.as_str(), "completed" | "failed" | "cancelled");
    let snapshot_axum: Event = snapshot.into();
    let connected_axum: Event = SSEWorkflowRunEvent::Connected(SSEConnectedData {
        message: format!("connected to workflow run {run_id}"),
        run_id,
    })
    .into();

    // Register a live client unless the run already reached a terminal
    // status (in which case we just replay Connected + Snapshot and close).
    // A single `stream!` block keeps the return type monomorphic — two
    // separate `stream!` invocations are distinct opaque types and can't
    // share one `impl Stream` signature.
    let live: Option<(Uuid, tokio::sync::mpsc::UnboundedReceiver<Result<Event, axum::Error>>)> =
        if terminal {
            None
        } else {
            let (tx, rx) =
                tokio::sync::mpsc::unbounded_channel::<Result<Event, axum::Error>>();
            let client_id = registry::register_client(run_id, tx).map_err(|e| {
                if e == "too many subscribers" {
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        AppError::new(
                            StatusCode::TOO_MANY_REQUESTS,
                            "WORKFLOW_TOO_MANY_SUBSCRIBERS",
                            e,
                        ),
                    )
                } else {
                    (
                        StatusCode::NOT_FOUND,
                        AppError::new(
                            StatusCode::NOT_FOUND,
                            "WORKFLOW_RUN_NOT_ACTIVE",
                            "run not currently active; refetch via GET /api/workflow-runs/{id}",
                        ),
                    )
                }
            })?;
            Some((client_id, rx))
        };

    let s = stream! {
        yield Ok::<Event, Infallible>(connected_axum);
        yield Ok::<Event, Infallible>(snapshot_axum);
        if let Some((client_id, mut rx)) = live {
            while let Some(item) = rx.recv().await {
                match item {
                    Ok(ev) => yield Ok::<Event, Infallible>(ev),
                    Err(_) => break,
                }
            }
            registry::remove_client(run_id, client_id);
        }
    };

    Ok((StatusCode::OK, Sse::new(s).keep_alive(KeepAlive::default())))
}

pub fn subscribe_docs(op: TransformOperation) -> TransformOperation {
    crate::modules::permissions::with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.subscribeRunEvents")
        .tag("Workflows - Runs")
        .summary("Subscribe to per-run progress events via SSE")
        .description(
            "Returns a snapshot frame followed by live per-step events until the run reaches a terminal status.",
        )
        .response::<200, axum::Json<SSEWorkflowRunEvent>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden"))
        .response_with::<404, (), _>(|r| r.description("Run not found"))
        .response_with::<429, (), _>(|r| r.description("Too many subscribers"))
}
