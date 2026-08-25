//! Shared "commit a new version from raw bytes" helper.
//!
//! Used by the `files_mcp` edit tools and the code-sandbox per-turn
//! version-back, so the process → save-blobs → append-version → emit-sync
//! sequence lives in exactly one place.

use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::models::{File, FileVersion, FileVersionCreateData};
use crate::modules::file::processing::{ProcessingManager, ProcessingResult};
use crate::modules::file::storage::manager::get_file_storage;

/// Append a new version of `file` from `new_bytes`.
///
/// Returns `Ok(None)` (a no-op) when the bytes are byte-identical to the head
/// (checksum match), so callers don't create empty versions. Otherwise:
/// re-processes (text pages / thumbnails), saves every blob keyed by the new
/// version id, appends the version (advancing head), and emits a sync event.
pub async fn commit_new_version(
    user_id: Uuid,
    file: &File,
    new_bytes: Vec<u8>,
    created_by: &str,
    source_message_id: Option<Uuid>,
) -> Result<Option<FileVersion>, AppError> {
    let storage = get_file_storage();
    let new_checksum = storage.calculate_checksum(&new_bytes);
    if file.checksum.as_deref() == Some(new_checksum.as_str()) {
        return Ok(None);
    }

    let ext = crate::modules::file::utils::extension_of(&file.filename);
    let mime = file
        .mime_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    // Degrade gracefully on a processing failure (same as the upload path) but
    // never SILENTLY: a swallowed error here would create a version with no
    // extracted text / thumbnails that looks like a binary file, with nothing in
    // the logs to explain it.
    let processing = match ProcessingManager::new().process_file(&new_bytes, &mime).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                error = %e,
                filename = %file.filename,
                "file versioning: processing failed; new version saved without extracted text/thumbnails"
            );
            ProcessingResult::default()
        }
    };

    let new_version_id = Uuid::new_v4();
    storage
        .save_original(user_id, new_version_id, &ext, &new_bytes)
        .await?;
    for (i, text) in processing.text_pages.iter().enumerate() {
        storage
            .save_text_page(user_id, new_version_id, (i + 1) as u32, text)
            .await?;
    }
    if let Some(thumb) = processing.thumbnails.first() {
        storage
            .save_image(user_id, new_version_id, 1, true, thumb)
            .await?;
    }
    for (i, img) in processing.images.iter().enumerate() {
        storage
            .save_image(user_id, new_version_id, (i + 1) as u32, false, img)
            .await?;
    }

    let version = Repos
        .file
        .append_version(
            file.id,
            new_version_id,
            FileVersionCreateData {
                file_size: new_bytes.len() as i64,
                mime_type: file.mime_type.clone(),
                checksum: Some(new_checksum),
                has_thumbnail: !processing.thumbnails.is_empty(),
                preview_page_count: processing.images.len() as i32,
                text_page_count: processing.text_pages.len() as i32,
                processing_metadata: serde_json::to_value(&processing.metadata)
                    .unwrap_or_default(),
                source_message_id,
                created_by: created_by.to_string(),
            },
        )
        .await?;

    super::sync::publish_file_changed(user_id, file.id);
    // Document RAG: re-index the new head version in the background.
    crate::modules::file_rag::ingest::spawn_reindex(user_id, file.id);
    Ok(Some(version))
}
