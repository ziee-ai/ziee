//! Shared file-store ingest: turn raw bytes into a durable, processed `File`
//! (originals blob + text/thumbnail derivatives + DB rows + cross-device sync).
//!
//! One code path for: workflow-run artifacts (A3), workflow tool-step
//! `resource_link` results (A6), and the chat MCP tool-result save path —
//! factored out of the previously-inline logic in `mcp/chat_extension/mcp.rs`.

use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::models::{File, FileCreateData};
use crate::modules::file::processing::ProcessingManager;
use crate::modules::file::storage::manager::get_file_storage;

/// Save `bytes` as a new durable file owned by `user_id`. Runs the processing
/// pipeline (text extraction + thumbnails), stores the original + derivatives,
/// creates the `files`/`file_versions` rows, optionally links the file to a
/// workflow run, and emits a cross-device `File` sync. Returns the head `File`.
#[allow(clippy::too_many_arguments)]
pub async fn ingest_bytes(
    user_id: Uuid,
    bytes: &[u8],
    filename: &str,
    mime_hint: Option<String>,
    created_by: &str,
    source_message_id: Option<Uuid>,
    workflow_run_id: Option<Uuid>,
) -> Result<File, AppError> {
    // Canonical extension (rsplit + lowercase) — MUST match how the download/
    // read paths derive the blob key (Path::extension would mis-key dotfiles).
    let ext = crate::modules::file::utils::extension_of(filename);
    let mime_type = mime_hint.or_else(|| mime_guess::from_ext(&ext).first().map(|m| m.to_string()));
    let mime_type_str = mime_type.as_deref().unwrap_or("application/octet-stream");

    let processing_result = ProcessingManager::new()
        .process_file(bytes, mime_type_str)
        .await
        .unwrap_or_default();

    let file_id = Uuid::new_v4();
    let storage = get_file_storage();
    storage
        .save_original(user_id, file_id, &ext, bytes)
        .await
        .map_err(|e| AppError::internal_error(format!("ingest: save_original: {e}")))?;

    for (n, text) in processing_result.text_pages.iter().enumerate() {
        let _ = storage
            .save_text_page(user_id, file_id, (n + 1) as u32, text)
            .await;
    }
    if let Some(thumb) = processing_result.thumbnails.first() {
        let _ = storage.save_image(user_id, file_id, 1, true, thumb).await;
    }
    for (n, img) in processing_result.images.iter().enumerate() {
        let _ = storage
            .save_image(user_id, file_id, (n + 1) as u32, false, img)
            .await;
    }

    let checksum = storage.calculate_checksum(bytes);
    let file = Repos
        .file
        .create(FileCreateData {
            id: file_id,
            user_id,
            filename: filename.to_string(),
            file_size: bytes.len() as i64,
            mime_type: mime_type.clone(),
            checksum: Some(checksum),
            has_thumbnail: !processing_result.thumbnails.is_empty(),
            preview_page_count: processing_result.images.len() as i32,
            text_page_count: processing_result.text_pages.len() as i32,
            processing_metadata: serde_json::to_value(&processing_result.metadata)
                .unwrap_or_default(),
            source_message_id,
            created_by: created_by.to_string(),
        })
        .await?;

    if let Some(run_id) = workflow_run_id {
        Repos.file.set_workflow_run_id(file_id, run_id).await?;
    }

    crate::modules::file::sync::publish_file_changed(user_id, file_id);

    Ok(file)
}
