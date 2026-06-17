// File version handlers — list versions, read a pinned version's metadata/bytes,
// and restore (append-only) a prior version as the new head.

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::handlers::download::{content_disposition, FILE_CONTENT_CACHE_CONTROL};
use crate::modules::file::models::{File, FileVersion};
use crate::modules::file::permissions::{FilesDownload, FilesPreview, FilesRead, FilesUpload};
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::types::{PreviewQuery, TextPageQuery};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;

/// Body for `POST /files/{id}/restore`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreVersionRequest {
    /// The version number to restore (a new head is appended with its bytes).
    pub version: i32,
}

/// List all versions of a file (newest first).
pub async fn list_versions(
    auth: RequirePermissions<(FilesRead,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Json<Vec<FileVersion>>> {
    let user_id = auth.user.id;
    // 404 if the file isn't the user's (don't leak existence).
    Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    let versions = Repos.file.list_versions(file_id, user_id).await?;
    Ok((StatusCode::OK, Json(versions)))
}

/// Get the current head version's metadata.
pub async fn get_head_version(
    auth: RequirePermissions<(FilesRead,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Json<FileVersion>> {
    let user_id = auth.user.id;
    let head = Repos
        .file
        .get_head(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    Ok((StatusCode::OK, Json(head)))
}

/// Get a specific version's metadata.
pub async fn get_version(
    auth: RequirePermissions<(FilesRead,)>,
    Path((file_id, version)): Path<(Uuid, i32)>,
) -> ApiResult<Json<FileVersion>> {
    let user_id = auth.user.id;
    let v = Repos
        .file
        .get_version(file_id, version, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File version"))?;
    Ok((StatusCode::OK, Json(v)))
}

/// Restore a prior version: append a new head whose bytes equal the target's.
/// No-op (returns the current head) if the target is already the head.
///
/// Gated on `FilesUpload`, not `FilesRead`/`FilesPreview`: restore is a WRITE —
/// it appends a new version and moves the file's head — so it requires the same
/// permission as uploading/editing, not merely reading. (The default `Users`
/// group has `files::upload`, so this is available to normal users.)
pub async fn restore_version(
    auth: RequirePermissions<(FilesUpload,)>,
    Path(file_id): Path<Uuid>,
    Json(req): Json<RestoreVersionRequest>,
) -> ApiResult<Json<File>> {
    let user_id = auth.user.id;
    let head = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // No-op: already at the requested version.
    if req.version == head.version {
        return Ok((StatusCode::OK, Json(head)));
    }
    // Reject a non-existent target with 400 (rather than silently appending).
    if Repos
        .file
        .get_version(file_id, req.version, user_id)
        .await?
        .is_none()
    {
        return Err(AppError::bad_request(
            "INVALID_VERSION",
            format!("version {} does not exist", req.version),
        )
        .into());
    }

    Repos
        .file
        .restore_version(file_id, req.version, "user".to_string(), None)
        .await?;

    // Sync: a restore changes the head.
    crate::modules::file::sync::publish_file_changed(user_id, file_id);

    // Document RAG: a restore makes a different version the head → re-index.
    crate::modules::file_rag::ingest::spawn_reindex(user_id, file_id);

    let updated = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    Ok((StatusCode::OK, Json(updated)))
}

/// Helper: resolve (file head/filename, target version) and load that version's
/// original bytes for the pinned download/preview/text endpoints.
async fn version_and_file(
    file_id: Uuid,
    version: i32,
    user_id: Uuid,
) -> Result<(File, FileVersion), AppError> {
    let file = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    let v = Repos
        .file
        .get_version(file_id, version, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File version"))?;
    Ok((file, v))
}

/// Download a specific version's original bytes.
pub async fn download_version(
    auth: RequirePermissions<(FilesDownload,)>,
    Path((file_id, version)): Path<(Uuid, i32)>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;
    let (file, v) = version_and_file(file_id, version, user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();
    let storage = get_file_storage();
    let bytes = storage
        .load_original(user_id, v.blob_version_id, &extension)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let headers = [
        (
            header::CONTENT_TYPE,
            v.mime_type
                .as_deref()
                .unwrap_or("application/octet-stream")
                .to_string(),
        ),
        (header::CONTENT_DISPOSITION, content_disposition(&file.filename)),
        (header::CONTENT_LENGTH, bytes.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];
    Ok((headers, bytes).into_response())
}

/// Get a specific version's preview image.
pub async fn preview_version(
    auth: RequirePermissions<(FilesPreview,)>,
    Path((file_id, version)): Path<(Uuid, i32)>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;
    let (_file, v) = version_and_file(file_id, version, user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let storage = get_file_storage();
    let image = storage
        .load_preview(user_id, v.blob_version_id, query.page)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let headers = [
        (header::CONTENT_TYPE, "image/jpeg".to_string()),
        (header::CONTENT_LENGTH, image.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];
    Ok((headers, image).into_response())
}

/// Get a specific version's extracted text.
pub async fn text_version(
    auth: RequirePermissions<(FilesRead,)>,
    Path((file_id, version)): Path<(Uuid, i32)>,
    Query(query): Query<TextPageQuery>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;
    let (_file, v) = version_and_file(file_id, version, user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let storage = get_file_storage();
    let text = match query.page {
        Some(page_num) => {
            if page_num < 1 || page_num > v.text_page_count as u32 {
                return Err(StatusCode::BAD_REQUEST);
            }
            storage
                .load_text_page(user_id, v.blob_version_id, page_num)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        None => {
            let mut out = String::new();
            for page_num in 1..=v.text_page_count {
                let page = storage
                    .load_text_page(user_id, v.blob_version_id, page_num as u32)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if page_num > 1 {
                    out.push_str("\n\n--- Page ");
                    out.push_str(&page_num.to_string());
                    out.push_str(" ---\n\n");
                }
                out.push_str(&page);
            }
            out
        }
    };
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
        (header::CONTENT_LENGTH, text.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];
    Ok((headers, text).into_response())
}

// ---- OpenAPI docs ----

pub fn list_versions_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesRead,)>(op)
        .id("File.listVersions")
        .tag("Files")
        .summary("List file versions")
        .response::<200, Json<Vec<FileVersion>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

pub fn get_head_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesRead,)>(op)
        .id("File.getHeadVersion")
        .tag("Files")
        .summary("Get the head version")
        .response::<200, Json<FileVersion>>()
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

pub fn get_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesRead,)>(op)
        .id("File.getVersion")
        .tag("Files")
        .summary("Get a specific file version")
        .response::<200, Json<FileVersion>>()
        .response_with::<404, (), _>(|res| res.description("File or version not found"))
}

pub fn restore_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesUpload,)>(op)
        .id("File.restore")
        .tag("Files")
        .summary("Restore a prior version (append-only)")
        .response::<200, Json<File>>()
        .response_with::<400, (), _>(|res| res.description("Invalid version"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

pub fn download_version_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;
    with_permission::<(FilesDownload,)>(op)
        .id("File.downloadVersion")
        .tag("Files")
        .summary("Download a specific version's bytes")
        .response::<200, Json<BlobType>>()
        .response_with::<404, (), _>(|res| res.description("File or version not found"))
}

pub fn preview_version_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;
    with_permission::<(FilesPreview,)>(op)
        .id("File.previewVersion")
        .tag("Files")
        .summary("Preview a specific version")
        .response::<200, Json<BlobType>>()
        .response_with::<404, (), _>(|res| res.description("File or version not found"))
}

pub fn text_version_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;
    with_permission::<(FilesRead,)>(op)
        .id("File.textVersion")
        .tag("Files")
        .summary("Get a specific version's extracted text")
        .response::<200, Json<BlobType>>()
        .response_with::<404, (), _>(|res| res.description("File or version not found"))
}
