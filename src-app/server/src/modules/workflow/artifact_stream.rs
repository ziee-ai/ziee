//! `GET /api/workflow-runs/{run_id}/artifact/{step_id}/{filename}` —
//! stream an artifact file's bytes.


use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::http::{header, StatusCode};
use axum::response::Response;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::workflow::artifact_io::artifact_host_path;
use crate::modules::workflow::permissions::WorkflowsRead;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::ArtifactMeta;

pub async fn read_artifact(
    auth: RequirePermissions<(WorkflowsRead,)>,
    AxumPath((run_id, step_id, filename)): AxumPath<(Uuid, String, String)>,
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
    // The artifact must have been collected (declared/`collect: all`) — the
    // persisted list is the allow-list; we never serve an arbitrary on-disk
    // file. `meta` supplies the content-type + length for the response.
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

    // Re-derive the host path from the current workspace layout rather than
    // trusting the persisted `meta.host_path` (a DB-stored absolute path).
    // `artifact_host_path` re-checks `step_id`/`filename` path-safety and
    // confines the result under the run's artifacts dir. The runner staged
    // artifacts at <workspace_root>/<conv_or_run>/workflow/<run>/artifacts/.
    let conv_dir_id = row.conversation_id.unwrap_or(run_id);
    let artifacts_dir = crate::modules::workflow::runner::workflow_workspace_root()
        .join(conv_dir_id.to_string())
        .join("workflow")
        .join(run_id.to_string())
        .join("artifacts");
    let path = artifact_host_path(&artifacts_dir, &step_id, &filename)?;

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
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
