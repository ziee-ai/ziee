//! `GET /api/workflow-runs/{run_id}/artifact/{step_id}/{filename}` —
//! stream an artifact file's bytes.

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
use crate::modules::workflow::types::ArtifactMeta;

pub async fn read_artifact(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath((run_id, step_id, filename)): AxumPath<(Uuid, String, String)>,
) -> ApiResult<Response> {
    if filename.contains("..") || filename.starts_with('/') {
        return Err::<_, (StatusCode, AppError)>((AppError::bad_request(
            "ARTIFACT_PATH_INVALID",
            "artifact filename not safe",
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
    let step_arts = row
        .step_artifacts_json
        .get(&step_id)
        .ok_or_else(|| AppError::not_found("step artifacts"))?;
    let arts: Vec<ArtifactMeta> = serde_json::from_value(step_arts.clone())
        .map_err(|e| AppError::internal_error(format!("decode artifact list: {e}")))?;
    let meta = arts
        .into_iter()
        .find(|m| m.filename == filename)
        .ok_or_else(|| AppError::not_found("artifact filename"))?;

    let bytes = tokio::fs::read(&meta.host_path).await.map_err(|e| {
        AppError::new(
            StatusCode::NOT_FOUND,
            "WORKFLOW_ARTIFACT_MISSING",
            format!("artifact file missing: {e}"),
        )
    })?;
    let body = Body::from(bytes);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, meta.mime_type)
        .header(header::CONTENT_LENGTH, meta.size_bytes.to_string())
        .body(body)
        .map_err(|e| AppError::internal_error(format!("response: {e}")))?;
    Ok((StatusCode::OK, resp))
}

pub fn read_artifact_docs(op: TransformOperation) -> TransformOperation {
    crate::modules::permissions::with_permission::<(WorkflowsRead,)>(op)
        .id("Workflow.readArtifact")
        .tag("Workflows - Runs")
        .summary("Read a step's artifact file content")
        .response::<200, axum::Json<serde_json::Value>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden"))
        .response_with::<404, (), _>(|r| r.description("Artifact not found"))
}
