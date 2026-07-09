// User-facing file export: download a file's head content converted to a chosen
// format (md/docx/pdf/odt/rtf/html) as an attachment. Distinct from the
// model-only `files_mcp::convert_document`, which SAVES a PDF back into the store
// — this streams a download in the user's chosen format and persists nothing.

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::handlers::download::content_disposition;
use crate::modules::file::permissions::FilesDownload;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::utils::export::{export_mime, render_to_format};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;

/// `?format=` for `GET /files/{id}/export`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportQuery {
    /// Target format: `md | docx | pdf | odt | rtf | html`.
    pub format: String,
}

/// Export a file's head content in a chosen format as a download.
///
/// `md` streams the raw source bytes; every other format is rendered by pandoc
/// (docx/odt/rtf/html are native writers; pdf uses the bundled typst engine).
/// Gated on `FilesDownload`; ownership-scoped (another user's id → 404).
pub async fn export_file(
    auth: RequirePermissions<(FilesDownload,)>,
    Path(file_id): Path<Uuid>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    let user_id = auth.user.id;
    let format = q.format.to_lowercase();
    let mime = export_mime(&format).ok_or_else(|| {
        AppError::bad_request(
            "INVALID_FORMAT",
            format!("unsupported export format '{}'", format),
        )
    })?;

    let file = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    let src_ext = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("md")
        .to_lowercase();
    let storage = get_file_storage();
    let bytes = storage
        .load_original(user_id, file.blob_version_id, &src_ext)
        .await
        .map_err(|_| AppError::not_found("File"))?;

    let stem = file
        .filename
        .rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| file.filename.clone());
    let out_name = format!("{}.{}", stem, format);

    let out_bytes = render_to_format(&bytes, &src_ext, &format).await?;

    let headers = [
        (header::CONTENT_TYPE, mime.to_string()),
        (header::CONTENT_DISPOSITION, content_disposition(&out_name)),
        (header::CONTENT_LENGTH, out_bytes.len().to_string()),
    ];
    Ok((StatusCode::OK, (headers, out_bytes).into_response()))
}

pub fn export_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesDownload,)>(op)
        .id("File.export")
        .tag("Files")
        .summary("Export a file")
        .description(
            "Download the file's head content converted to md/docx/pdf/odt/rtf/html \
             as an attachment.",
        )
}
