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

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024; // 100MB

/// Upload file handler
pub async fn upload_file(
    auth: RequirePermissions<(FilesUpload,)>,
    mut multipart: Multipart,
) -> ApiResult<Json<File>> {
    let user_id = auth.user.id;

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

    // Validate file size
    if file_data.len() > MAX_FILE_SIZE {
        return Err(AppError::bad_request(
            "FILE_TOO_LARGE",
            format!("File size exceeds maximum of {} bytes", MAX_FILE_SIZE),
        ).into());
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
        return Err(AppError::bad_request("MIME_MISMATCH", reason).into());
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
    if crate::modules::file::utils::zipbomb::is_ooxml_or_odf(mime_type_str) {
        if let Err(e) = crate::modules::file::utils::zipbomb::validate(&file_data) {
            return Err(AppError::bad_request("ZIP_BOMB_DETECTED", e.to_string()).into());
        }
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

    // Create database record
    let file = Repos.file
        .create(FileCreateData {
            id: file_id,
            user_id,
            filename,
            file_size: file_data.len() as i64,
            mime_type,
            checksum: Some(checksum),
            has_thumbnail: !processing_result.thumbnails.is_empty(),
            preview_page_count: processing_result.images.len() as i32,
            text_page_count: processing_result.text_pages.len() as i32,
            processing_metadata: serde_json::to_value(&processing_result.metadata)
                .unwrap_or(serde_json::json!({})),
            created_by: "user".to_string(),
        })
        .await?;

    // Emit event (async)
    // Note: EventBus integration would go here

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
        .response_with::<403, (), _>(|res| res.description("Forbidden"))
}
