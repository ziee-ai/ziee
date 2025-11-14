// File management handlers

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::models::File;
use crate::modules::file::permissions::{FilesDelete, FilesPreview, FilesRead};
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::types::{FileListResponse, PaginationQuery, PreviewQuery};
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

/// Get file preview/thumbnail
pub async fn get_preview(
    auth: RequirePermissions<(FilesPreview,)>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> Result<(HeaderMap, Vec<u8>), AppError> {
    let user_id = auth.user.id;

    // Verify file ownership
    Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Load thumbnail
    let storage = get_file_storage();
    let thumbnail_data = storage
        .load_image(user_id, file_id, query.page, true)
        .await?;

    // Build response
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
    headers.insert(
        header::CONTENT_LENGTH,
        thumbnail_data.len().to_string().parse().unwrap(),
    );

    Ok((headers, thumbnail_data))
}

/// Get extracted text content
pub async fn get_text_content(
    auth: RequirePermissions<(FilesRead,)>,
    Path(file_id): Path<Uuid>,
) -> Result<(HeaderMap, String), AppError> {
    let user_id = auth.user.id;

    // Verify file ownership
    Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Load text content
    let storage = get_file_storage();
    let text_content = storage.load_text(user_id, file_id).await?;

    // Build response
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/plain; charset=utf-8".parse().unwrap());
    headers.insert(
        header::CONTENT_LENGTH,
        text_content.len().to_string().parse().unwrap(),
    );

    Ok((headers, text_content))
}

/// Delete file
pub async fn delete_file(
    auth: RequirePermissions<(FilesDelete,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let user_id = auth.user.id;

    // Delete from database
    Repos.file.delete(file_id, user_id).await?;

    // Delete from storage
    let storage = get_file_storage();
    storage.delete_all(user_id, file_id).await?;

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
