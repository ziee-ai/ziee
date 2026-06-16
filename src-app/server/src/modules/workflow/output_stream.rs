//! `GET /api/workflow-runs/{run_id}/output/{step_id}` — stream a step
//! output file's content as text.

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
use crate::modules::workflow::types::OutputMeta;

pub async fn read_output(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath((run_id, step_id)): AxumPath<(Uuid, String)>,
) -> ApiResult<Response> {
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
    let meta_json = row
        .step_outputs_json
        .get(&step_id)
        .ok_or_else(|| AppError::not_found("step output"))?;
    let meta: OutputMeta = serde_json::from_value(meta_json.clone())
        .map_err(|e| AppError::internal_error(format!("decode step meta: {e}")))?;
    let bytes = tokio::fs::read(&meta.path).await.map_err(|e| {
        AppError::new(
            StatusCode::NOT_FOUND,
            "WORKFLOW_OUTPUT_MISSING",
            format!("output file missing: {e}"),
        )
    })?;
    let body = Body::from(bytes);
    let ct = match meta.parsed_as {
        crate::modules::workflow::types::ParsedAs::Json => "application/json",
        crate::modules::workflow::types::ParsedAs::Text => "text/plain; charset=utf-8",
    };
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ct)
        .header(header::CONTENT_LENGTH, meta.size_bytes.to_string())
        .body(body)
        .map_err(|e| AppError::internal_error(format!("response: {e}")))?;
    Ok((StatusCode::OK, resp))
}

pub fn read_output_docs(op: TransformOperation) -> TransformOperation {
    crate::modules::permissions::with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.readOutput")
        .tag("Workflows - Runs")
        .summary("Read a step's output file content")
        .response::<200, axum::Json<serde_json::Value>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden"))
        .response_with::<404, (), _>(|r| r.description("Output not found"))
}
