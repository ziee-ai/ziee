// File management handlers

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::handlers::download::FILE_CONTENT_CACHE_CONTROL;
use crate::modules::file::models::File;
use crate::modules::file::permissions::{FilesDelete, FilesPreview, FilesRead};
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::types::{FileListResponse, PaginationQuery, PreviewQuery, TextPageQuery};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;
use uuid::Uuid;

/// List user's files
pub async fn list_files(
    auth: RequirePermissions<(FilesRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<FileListResponse>> {
    let user_id = auth.user.id;

    let (files, total) = Repos.file
        .list_by_user(user_id, params.page, params.per_page)
        .await?;

    Ok((
        StatusCode::OK,
        Json(FileListResponse {
            files,
            total,
            page: params.page,
            per_page: params.per_page,
        }),
    ))
}

/// Get file metadata
pub async fn get_file(
    auth: RequirePermissions<(FilesRead,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Json<File>> {
    let user_id = auth.user.id;

    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    Ok((StatusCode::OK, Json(file)))
}

/// Get file preview image (high quality)
pub async fn get_preview(
    auth: RequirePermissions<(FilesPreview,)>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;

    // Verify file ownership + resolve head blob.
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Load high-quality preview image
    let storage = get_file_storage();
    let image_data = storage
        .load_preview(user_id, file.blob_version_id, query.page)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Private, bounded cache (see FILE_CONTENT_CACHE_CONTROL) so reloads reuse
    // preview bytes without re-downloading every inline image — the main fix
    // for laggy reloads with many inline previews.
    let headers = [
        (header::CONTENT_TYPE, "image/jpeg".to_string()),
        (header::CONTENT_LENGTH, image_data.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];

    Ok((headers, image_data).into_response())
}

/// Get file thumbnail (300px, single thumbnail from first page)
pub async fn get_thumbnail(
    auth: RequirePermissions<(FilesPreview,)>,
    Path(file_id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;

    // Verify file ownership + resolve head blob.
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Load thumbnail
    let storage = get_file_storage();
    let thumbnail_data = storage
        .load_thumbnail(user_id, file.blob_version_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Private, bounded cache (see FILE_CONTENT_CACHE_CONTROL) — reuse across
    // reloads without a round-trip.
    let headers = [
        (header::CONTENT_TYPE, "image/jpeg".to_string()),
        (header::CONTENT_LENGTH, thumbnail_data.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];

    Ok((headers, thumbnail_data).into_response())
}

/// Get extracted text content
pub async fn get_text_content(
    auth: RequirePermissions<(FilesRead,)>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<TextPageQuery>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;

    // Verify file ownership and get file info
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let storage = get_file_storage();
    let text_content = match query.page {
        Some(page_num) => {
            // Return specific page
            if page_num < 1 || page_num > file.text_page_count as u32 {
                return Err(StatusCode::BAD_REQUEST);
            }
            storage
                .load_text_page(user_id, file.blob_version_id, page_num)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        None => {
            // Return all pages concatenated
            let mut text_content = String::new();
            for page_num in 1..=file.text_page_count {
                let page_text = storage
                    .load_text_page(user_id, file.blob_version_id, page_num as u32)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if page_num > 1 {
                    text_content.push_str("\n\n--- Page ");
                    text_content.push_str(&page_num.to_string());
                    text_content.push_str(" ---\n\n");
                }
                text_content.push_str(&page_text);
            }
            text_content
        }
    };

    // Private, bounded cache (see FILE_CONTENT_CACHE_CONTROL) — reuse across
    // reloads without a round-trip.
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
        (header::CONTENT_LENGTH, text_content.len().to_string()),
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];

    Ok((headers, text_content).into_response())
}

/// Delete file
pub async fn delete_file(
    auth: RequirePermissions<(FilesDelete,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let user_id = auth.user.id;

    // Delete from database (returns the DISTINCT blob_version_ids to purge).
    let blob_ids = Repos.file.delete(file_id, user_id).await?;

    // Delete each distinct version blob from storage. Restored versions share a
    // blob, so the repo already deduped — no double-delete / missing-blob.
    let storage = get_file_storage();
    for blob_id in blob_ids {
        storage.delete_all(user_id, blob_id).await?;
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

/// List files OpenAPI documentation
pub fn list_files_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesRead,)>(op)
        .id("File.list")
        .tag("Files")
        .summary("List user's files")
        .description("Get paginated list of files uploaded by the current user")
        .response::<200, Json<FileListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get file OpenAPI documentation
pub fn get_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesRead,)>(op)
        .id("File.get")
        .tag("Files")
        .summary("Get file metadata")
        .description("Retrieve metadata for a specific file")
        .response::<200, Json<File>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

/// Get preview OpenAPI documentation
pub fn get_preview_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;

    with_permission::<(FilesPreview,)>(op)
        .id("File.getPreview")
        .tag("Files")
        .summary("Get preview image")
        .description("Get high-quality preview image for a specific page (2000px height)")
        .response::<200, Json<BlobType>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File or preview not found"))
}

/// Get thumbnail OpenAPI documentation
pub fn get_thumbnail_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;

    with_permission::<(FilesPreview,)>(op)
        .id("File.getThumbnail")
        .tag("Files")
        .summary("Get thumbnail")
        .description("Get thumbnail image from first page (300px height)")
        .response::<200, Json<BlobType>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File or thumbnail not found"))
}

/// Get text content OpenAPI documentation
pub fn get_text_content_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;

    with_permission::<(FilesRead,)>(op)
        .id("File.getTextContent")
        .tag("Files")
        .summary("Get extracted text")
        .description("Get extracted text content for a specific page or all pages")
        .response::<200, Json<BlobType>>()
        .response_with::<400, (), _>(|res| res.description("Invalid page number"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

/// Delete file OpenAPI documentation
pub fn delete_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesDelete,)>(op)
        .id("File.delete")
        .tag("Files")
        .summary("Delete file")
        .description("Delete a file and all associated data (thumbnails, extracted text, etc.)")
        .response_with::<204, (), _>(|res| res.description("File deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}
