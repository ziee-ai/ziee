//! Domain orchestration over the `ziee-file` store's ingest path.
//!
//! The STORE half (process → save blobs → create rows → change-event) lives in
//! `ziee_file::ingest`, driven by two injected seams. This module supplies the
//! ziee implementations of those seams — [`ZieeFileProcessor`] (the real
//! `ProcessingManager`) and [`ZieeFileEvents`] (the real `sync::publish`) — and
//! keeps the app-domain `workflow_run_id` linkage (now a `file_workflow_runs`
//! join row) that the store deliberately does not carry.

use async_trait::async_trait;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::models::{File, ProcessingResult};
use crate::modules::file::processing::ProcessingManager;
use ziee_file::seams::{FileEvents, FileProcessor};

/// ziee's [`FileProcessor`] seam impl — the real extraction engine.
pub struct ZieeFileProcessor;

#[async_trait]
impl FileProcessor for ZieeFileProcessor {
    async fn process(&self, bytes: &[u8], mime_type: &str) -> Result<ProcessingResult, AppError> {
        ProcessingManager::new().process_file(bytes, mime_type).await
    }
}

/// ziee's [`FileEvents`] seam impl — routes store change/delete notifications
/// to the concrete owner-scoped `SyncEntity::File` publish functions.
pub struct ZieeFileEvents;

impl FileEvents for ZieeFileEvents {
    fn on_file_changed(&self, user_id: Uuid, file_id: Uuid, origin: Option<Uuid>) {
        crate::modules::file::sync::publish_file_changed_with_origin(user_id, file_id, origin);
    }
    fn on_file_deleted(&self, user_id: Uuid, file_id: Uuid, origin: Option<Uuid>) {
        crate::modules::file::sync::publish_file_deleted_with_origin(user_id, file_id, origin);
    }
    fn on_committed(&self, user_id: Uuid, file_id: Uuid, is_new: bool) {
        // Document RAG. The HTTP restore path (the sole `on_committed` caller
        // moved in chunk `ziee-file-http`) commits a new head of an EXISTING
        // file → `spawn_reindex`. Brand-new-file indexing (`spawn_index`, which
        // needs the full `&File`) stays on the upload path and calls it directly,
        // so `is_new == true` is never reached through this seam.
        if !is_new {
            crate::modules::file_rag::ingest::spawn_reindex(user_id, file_id);
        }
    }
}

/// Assemble the [`ziee_file::http::FileContext`] the mountable file handlers pull
/// from the request extensions, installing ziee's [`ZieeFileEvents`] seam impl +
/// the download-token signing material (issuer/secret from the app's JWT
/// config). Called once at boot by `lib.rs` (and thus the desktop embed).
pub fn build_file_context(
    pool: std::sync::Arc<sqlx::PgPool>,
    jwt: &crate::core::config::JwtConfig,
) -> ziee_file::http::FileContext {
    ziee_file::http::FileContext {
        files: std::sync::Arc::new(ziee_file::FileRepository::new((*pool).clone())),
        events: std::sync::Arc::new(ZieeFileEvents),
        download_token: ziee_file::http::DownloadTokenSigner {
            issuer: jwt.issuer.clone(),
            secret: jwt.secret.clone(),
        },
    }
}

/// Save `bytes` as a new durable file owned by `user_id`. Runs the processing
/// pipeline (text extraction + thumbnails) via the store's seam, stores the
/// original + derivatives, creates the `files`/`file_versions` rows, optionally
/// links the file to a workflow run (via the app-side `file_workflow_runs` join
/// table), and emits a cross-device `File` sync. Returns the head `File`.
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
    let file = ziee_file::ingest::ingest_bytes(
        &Repos.file,
        &ZieeFileProcessor,
        &ZieeFileEvents,
        user_id,
        bytes,
        filename,
        mime_hint,
        created_by,
        source_message_id,
    )
    .await?;

    if let Some(run_id) = workflow_run_id {
        Repos.file_workflow_runs.link(file.id, run_id).await?;
    }

    Ok(file)
}
