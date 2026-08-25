// File version — the ziee-RETAINED write handler.
//
// Chunk `ziee-file-http` moved the store-generic version handlers (list / head /
// get / restore / pinned download+preview+text) into `ziee_file::http`. This
// module keeps only `append_version`: it appends a new head from user text via
// `versioning::commit_new_version`, which runs the `ProcessingManager` producer
// (text extraction / thumbnails) — a processing-coupled path the store does not
// carry — so it stays ziee-side.

use aide::transform::TransformOperation;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::models::File;
use crate::modules::file::permissions::FilesUpload;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;

/// Body for `POST /files/{id}/versions`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendVersionRequest {
    /// New full text content for the file. A new head version is appended;
    /// byte-identical content is a no-op.
    pub content: String,
}

/// Append a new head version from user-supplied text content — the user side of
/// co-editing a deliverable. A byte-identical body is a no-op (returns the
/// current head unchanged).
///
/// Gated on `FilesUpload` (a WRITE, like `restore_version`). Ownership-scoped:
/// another user's file id resolves to 404.
pub async fn append_version(
    auth: RequirePermissions<(FilesUpload,)>,
    Path(file_id): Path<Uuid>,
    Json(req): Json<AppendVersionRequest>,
) -> ApiResult<Json<File>> {
    let user_id = auth.user.id;
    let file = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Append-only: `commit_new_version` returns None when the bytes match the
    // current head (content-addressed no-op), so a "save" with no change never
    // creates an empty version. It ALSO emits the `SyncEntity::File` change +
    // spawns the RAG reindex internally on a successful commit — so we must NOT
    // do either again here (that would double the sync events + reindex work).
    crate::modules::file::versioning::commit_new_version(
        user_id,
        &file,
        req.content.into_bytes(),
        "user",
        None,
    )
    .await?;

    let updated = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    Ok((StatusCode::OK, Json(updated)))
}

pub fn append_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesUpload,)>(op)
        .id("File.appendVersion")
        .tag("Files")
        .summary("Append a file version")
        .description(
            "Append a new head version from user-supplied text content (the user side of \
             co-editing a deliverable). Byte-identical content is a no-op.",
        )
}
