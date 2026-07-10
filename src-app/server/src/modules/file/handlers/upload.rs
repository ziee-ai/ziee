// File upload handler

use aide::transform::TransformOperation;
use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::Json;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::models::{File, FileCreateData};
use crate::modules::file::permissions::FilesUpload;
use crate::modules::file::processing::{ProcessingManager, ProcessingResult};
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;
use uuid::Uuid;

/// Per-user storage quota for uploads. Closes 05-file F-16 (Medium). 10 GiB
/// matches typical SaaS chat-attachment quotas. Exposed as a constant so
/// other upload entry points (e.g. project file upload) enforce the same
/// cap.
pub const PER_USER_STORAGE_QUOTA_BYTES: i64 = 10 * 1024 * 1024 * 1024; // 10 GiB

/// Core upload routine used by every upload entry point.
///
/// Owns: multipart parsing, size + quota checks, MIME sniffing,
/// zip-bomb validation, processing (thumbnail/text extraction),
/// disk save, and DB row creation. Caller owns: permission gating +
/// any post-create wiring (e.g. attaching the new file to a project).
///
/// **SECURITY CRITICAL — caller responsibility**: `user_id` is taken
/// as a parameter instead of being extracted from the auth context
/// inside this function. The new file row is owned by `user_id`, and
/// per-user quota is checked against `user_id`. The caller MUST pass
/// the authenticated user's id (via `RequirePermissions<…>.user.id`)
/// — passing any other UUID would let one user upload as another and
/// inflate that user's quota. This function does NOT re-validate the
/// id against the request's auth context.
///
/// Returns the freshly-created `File` so the caller can use its `id`
/// for follow-on actions (the project upload-and-attach endpoint
/// inserts into `project_files` immediately after this returns).
pub async fn upload_file_inner(
    user_id: Uuid,
    mut multipart: Multipart,
    origin: Option<Uuid>,
) -> Result<File, AppError> {
    // Extract file from multipart
    let mut filename: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "file" {
            filename = field.file_name().map(|s| s.to_string());
            file_data = Some(field.bytes().await.map_err(|e| {
                AppError::bad_request("UPLOAD_ERROR", format!("Failed to read file: {}", e))
            })?.to_vec());
        }
    }

    let filename = filename.ok_or_else(|| {
        AppError::bad_request("MISSING_FILE", "No file provided in upload")
    })?;

    let file_data = file_data.ok_or_else(|| {
        AppError::bad_request("MISSING_FILE_DATA", "No file data provided")
    })?;

    // Validate file size against the configurable per-file cap
    // (`config.server.max_file_upload_mb`, captured at boot). Both this handler
    // and the per-route body-limit layer read the same source of truth.
    let max_file_size = crate::core::get_max_file_upload_bytes();
    if file_data.len() > max_file_size {
        // Report the file size with one decimal (raw bytes → MB) so a file just
        // over the cap doesn't render as "1 MB exceeds 1 MB"; use MB in both the
        // backend and the UI for a consistent user-facing unit.
        return Err(AppError::bad_request(
            "FILE_TOO_LARGE",
            format!(
                "File size {:.1} MB exceeds the maximum upload size of {} MB",
                file_data.len() as f64 / (1024.0 * 1024.0),
                max_file_size / (1024 * 1024),
            ),
        ));
    }

    // Per-user storage quota (see PER_USER_STORAGE_QUOTA_BYTES above).
    let used = Repos.file.count_user_bytes(user_id).await?;
    let after = used.saturating_add(file_data.len() as i64);
    if after > PER_USER_STORAGE_QUOTA_BYTES {
        return Err(AppError::bad_request(
            "STORAGE_QUOTA_EXCEEDED",
            format!(
                "Upload would put you over the {} GiB per-user storage quota \
                 ({} bytes already used + {} bytes incoming)",
                PER_USER_STORAGE_QUOTA_BYTES / (1024 * 1024 * 1024),
                used,
                file_data.len()
            ),
        ));
    }

    // Generate file ID
    let file_id = Uuid::new_v4();

    // Extract extension
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Determine MIME type. Extension is the starting point, but we
    // sniff the actual bytes to reject HTML-disguised-as-image-or-pdf
    // smuggling (browsers may sniff and render HTML in the user's
    // origin → stored XSS). Closes 05-file F-04 (High).
    let claimed_mime = mime_guess::from_ext(&extension)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let sniffed_mime = crate::modules::file::utils::magic::sniff_mime(&file_data);
    if let Some(reason) =
        crate::modules::file::utils::magic::smuggling_rejection(sniffed_mime, &claimed_mime)
    {
        return Err(AppError::bad_request("MIME_MISMATCH", reason));
    }
    // Prefer the sniffed MIME when known AND it provides more
    // specificity than the extension-derived value. For
    // application/zip-family containers (DOCX/XLSX/PPTX/ODT) the
    // extension-derived MIME is MORE specific than the sniffed
    // generic "application/zip", so we keep the claimed one in that
    // case — preserving downstream processor dispatch.
    let mime_type = Some(match sniffed_mime {
        Some("application/zip") => claimed_mime,
        Some(s) => s.to_string(),
        None => claimed_mime,
    });

    // Get storage and calculate checksum
    let storage = get_file_storage();
    let checksum = storage.calculate_checksum(&file_data);

    // Process file
    let processing_manager = ProcessingManager::new();
    let mime_type_str = mime_type.as_deref().unwrap_or("application/octet-stream");
    tracing::info!("Processing file with MIME type: {}", mime_type_str);

    // Decompression-bomb pre-validation for OOXML/ODF containers.
    // Closes 05-file F-05 (High). For non-ZIP-family MIMEs this is a
    // no-op via the is_ooxml_or_odf gate.
    if crate::modules::file::utils::zipbomb::is_ooxml_or_odf(mime_type_str)
        && let Err(e) = crate::modules::file::utils::zipbomb::validate(&file_data) {
            return Err(AppError::bad_request("ZIP_BOMB_DETECTED", e.to_string()));
        }

    let processing_result = match processing_manager
        .process_file(&file_data, mime_type_str)
        .await
    {
        Ok(result) => {
            tracing::info!(
                "File processing successful: {} thumbnails, {} images, {} text pages",
                result.thumbnails.len(),
                result.images.len(),
                result.text_pages.len()
            );
            result
        }
        Err(e) => {
            tracing::error!("File processing failed: {}", e);
            ProcessingResult::default()
        }
    };

    // Save original file
    storage
        .save_original(user_id, file_id, &extension, &file_data)
        .await?;

    // Save extracted text pages
    for (page_num, text) in processing_result.text_pages.iter().enumerate() {
        storage
            .save_text_page(user_id, file_id, (page_num + 1) as u32, text)
            .await?;
    }

    // Save single thumbnail (page_num parameter ignored for thumbnails, but pass 1 for consistency)
    if let Some(thumbnail_data) = processing_result.thumbnails.first() {
        storage
            .save_image(user_id, file_id, 1, true, thumbnail_data)
            .await?;
    }

    // Save high-quality images
    for (idx, image_data) in processing_result.images.iter().enumerate() {
        storage
            .save_image(user_id, file_id, (idx + 1) as u32, false, image_data)
            .await?;
    }

    // Upload-time suitability advisory (Track A §2b). Non-blocking: every upload
    // still succeeds; we just annotate `processing_metadata` so the UI can warn
    // the user that a file type reads poorly and suggest a better format. Computed
    // empirically from the actual extraction result, so scanned PDFs / failed
    // extractions are caught too — not just by mime.
    let mut processing_metadata = serde_json::to_value(&processing_result.metadata)
        .unwrap_or(serde_json::json!({}));
    {
        let has_text = !processing_result.text_pages.is_empty();
        let (suitability, suggestion) = file_suitability(mime_type_str, has_text);
        if let Some(obj) = processing_metadata.as_object_mut() {
            obj.insert("suitability".to_string(), serde_json::json!(suitability));
            if let Some(s) = suggestion {
                obj.insert("suggestion".to_string(), serde_json::json!(s));
            }
        }
    }

    // Create database record
    let file_create = FileCreateData {
        id: file_id,
        user_id,
        filename,
        file_size: file_data.len() as i64,
        mime_type,
        checksum: Some(checksum),
        has_thumbnail: !processing_result.thumbnails.is_empty(),
        preview_page_count: processing_result.images.len() as i32,
        text_page_count: processing_result.text_pages.len() as i32,
        processing_metadata,
        source_message_id: None,
        created_by: "user".to_string(),
    };
    // Atomic quota guard (closes the TOCTOU between the pre-check above and the
    // insert). On a lost race the blob is already on disk — remove it so a
    // quota rejection doesn't leave an orphan.
    let file = match Repos.file.create_with_quota(file_create, PER_USER_STORAGE_QUOTA_BYTES).await {
        Ok(f) => f,
        Err(e) => {
            let _ = storage.delete_all(user_id, file_id).await;
            return Err(e);
        }
    };

    // Notify the owner's other devices a new file exists (I3). Emitted from the
    // shared core so BOTH the direct `/files/upload` and the project
    // `upload+attach` paths sync. `origin` skips the uploading device's own
    // redundant refetch when the request carried its SSE connection id.
    crate::modules::file::sync::publish_file_changed_with_origin(user_id, file.id, origin);

    // Document RAG: chunk + (when an embedder is configured) embed in the
    // background. Self-gates on file_rag_admin_settings.enabled.
    crate::modules::file_rag::ingest::spawn_index(user_id, &file);

    Ok(file)
}

/// Classify a file's LLM-suitability for the upload advisory. Returns
/// `("good"|"low", Option<suggestion>)`. Images and any file with extracted text
/// are "good"; everything else gets a type-specific nudge toward a better format.
fn file_suitability(mime: &str, has_text: bool) -> (&'static str, Option<&'static str>) {
    if mime.starts_with("image/") || has_text {
        return ("good", None);
    }
    if mime == "application/pdf" {
        return (
            "low",
            Some("This PDF has no text layer (scanned) — upload a text-based or OCR'd PDF."),
        );
    }
    if mime.contains("presentationml") || mime == "application/vnd.ms-powerpoint" {
        return (
            "low",
            Some("PowerPoint reads poorly — export to PDF and upload that for best results."),
        );
    }
    if mime == "application/zip"
        || mime == "application/gzip"
        || mime == "application/x-tar"
        || mime.contains("compressed")
    {
        return (
            "low",
            Some("Archives can't be read — upload the files individually."),
        );
    }
    if mime.starts_with("audio/") || mime.starts_with("video/") {
        return (
            "low",
            Some("Media files aren't read — upload a transcript instead."),
        );
    }
    ("low", Some("This file type can't be read by the assistant."))
}


/// Upload file handler — thin wrapper around `upload_file_inner` that
/// adds permission gating + the 201 response code.
pub async fn upload_file(
    auth: RequirePermissions<(FilesUpload,)>,
    origin: crate::modules::sync::SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<File>> {
    let file = upload_file_inner(auth.user.id, multipart, origin.0).await?;
    Ok((StatusCode::CREATED, Json(file)))
}

/// Upload file OpenAPI documentation
pub fn upload_file_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesUpload,)>(op)
        .id("File.upload")
        .tag("Files")
        .summary("Upload a file")
        .description("Upload a file for storage and processing. Supports text extraction and thumbnail generation.")
        .response::<201, Json<File>>()
        .response_with::<400, (), _>(|res| {
            res.description("Bad Request - Invalid file or file too large")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
#[cfg(test)]
mod suitability_tests {
    use super::file_suitability;


    #[test]
    fn images_and_text_are_good() {
        assert_eq!(file_suitability("image/png", false).0, "good");
        assert_eq!(file_suitability("text/markdown", true).0, "good");
        assert_eq!(file_suitability("application/pdf", true).0, "good");
    }


    #[test]
    fn powerpoint_suggests_pdf() {
        let (s, sug) = file_suitability(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            false,
        );
        assert_eq!(s, "low");
        assert!(sug.unwrap().contains("PDF"));
    }


    #[test]
    fn scanned_pdf_archive_media_are_low() {
        assert_eq!(file_suitability("application/pdf", false).0, "low");
        assert_eq!(file_suitability("application/zip", false).0, "low");
        assert_eq!(file_suitability("audio/mpeg", false).0, "low");
        assert_eq!(file_suitability("application/octet-stream", false).0, "low");
    }
}
