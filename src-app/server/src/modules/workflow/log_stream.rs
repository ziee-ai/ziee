//! `GET /api/workflow-runs/{run_id}/logs/{step_id}/{kind}` — read a
//! per-step log file (also the backing for workflow_mcp `resources/read`
//! on `ziee://workflow-runs/<run>/logs/<step>/<kind>` URIs; B5).
//!
//! `kind` is one of: `prompt | raw_output | stderr | trace`. The
//! per-item llm_map logs are at `kind = items/<N>` (one extra path
//! segment); B5's workflow_mcp `resources/read` is the primary
//! consumer for those.

#![allow(dead_code)]

use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::http::{header, StatusCode};
use axum::response::Response;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::workflow::permissions::WorkflowsRead;
use crate::modules::workflow::repository;

const ALLOWED_KINDS: &[&str] = &["prompt", "raw_output", "stderr", "trace"];

pub async fn read_log(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath((run_id, step_id, kind)): AxumPath<(Uuid, String, String)>,
) -> ApiResult<Response> {
    if !ALLOWED_KINDS.contains(&kind.as_str()) {
        return Err::<_, (StatusCode, AppError)>((AppError::bad_request(
            "WORKFLOW_LOG_BAD_KIND",
            format!(
                "log kind '{kind}' not recognized (allowed: {})",
                ALLOWED_KINDS.join(", ")
            ),
        )).into());
    }
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

    // Resolve the on-disk path. The runner staged logs under
    // <workspace_root>/<conv_or_run>/workflow/<run>/logs/<step>/<kind>(.json).
    let conv_dir_id = row.conversation_id.unwrap_or(run_id);
    let workspace_root = crate::modules::workflow::runner::workflow_workspace_root();
    let base = workspace_root
        .join(conv_dir_id.to_string())
        .join("workflow")
        .join(run_id.to_string())
        .join("logs")
        .join(&step_id);
    let path = if kind == "trace" {
        base.join("trace.json")
    } else {
        base.join(&kind)
    };

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        AppError::new(
            StatusCode::NOT_FOUND,
            "WORKFLOW_LOG_MISSING",
            format!("log file missing: {e}"),
        )
    })?;
    let total_len = bytes.len() as u64;
    let body = Body::from(bytes);
    let ct = if kind == "trace" {
        "application/json"
    } else {
        "text/plain; charset=utf-8"
    };
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ct)
        .header(header::CONTENT_LENGTH, total_len.to_string())
        .body(body)
        .map_err(|e| AppError::internal_error(format!("response: {e}")))?;
    Ok((StatusCode::OK, resp))
}

pub fn read_log_docs(op: TransformOperation) -> TransformOperation {
    crate::modules::permissions::with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.readLog")
        .tag("Workflows - Runs")
        .summary("Read a step's diagnostic log (prompt / raw_output / stderr / trace)")
        .response::<200, axum::Json<serde_json::Value>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden"))
        .response_with::<404, (), _>(|r| r.description("Log not found"))
}
