//! One-shot geometry backfill (FB-9). Captures per-page citation geometry for
//! files that were ingested before geometry existed (or that failed capture),
//! so their chat citations get an exact-passage highlight instead of only a
//! page-level deep-link. Mirrors `file_rag::run_backfill`: single-flight,
//! bounded per boot, skips files that already have geometry.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::database::get_database_pool;
use crate::modules::file::processing::ProcessingManager;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::utils::extension_of;

static IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Cap per boot so a large historical corpus is spread across restarts rather
/// than blocking one startup (office re-conversion is not free).
const BACKFILL_LIMIT: i64 = 500;

/// Run the backfill once (no-op if another run is already in flight).
pub async fn run_geometry_backfill() {
    if IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    // Reset the single-flight flag on drop — so a panic inside `run_inner`
    // (e.g. a processor panic on a malformed PDF) can't wedge the flag `true`
    // and permanently disable future backfills for the process.
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            IN_PROGRESS.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    run_inner().await;
}

async fn run_inner() {
    let pool = match get_database_pool() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("geometry.backfill: no db pool: {e}");
            return;
        }
    };
    let pool = pool.as_ref();
    // Only the mimes that can carry per-char geometry (PDF + Word-style office),
    // head version, with extracted text. Newest first.
    let rows = match sqlx::query!(
        r#"
        SELECT f.id                AS "file_id!",
               f.user_id           AS "user_id!",
               f.filename          AS "filename!",
               f.mime_type,
               fv.blob_version_id  AS "blob_version_id!"
        FROM files f
        JOIN file_versions fv ON fv.id = f.current_version_id
        WHERE f.text_page_count > 0
          AND f.mime_type IN (
            'application/pdf',
            'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            'application/msword', 'application/rtf', 'text/rtf',
            'application/vnd.oasis.opendocument.text'
          )
        ORDER BY f.created_at DESC
        LIMIT $1
        "#,
        BACKFILL_LIMIT,
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("geometry.backfill: query failed: {e}");
            return;
        }
    };

    let storage = get_file_storage();
    let pm = ProcessingManager::new();
    let mut done = 0usize;

    for r in rows {
        // Skip files that already have geometry (probe page 1 — keyed by the
        // head blob_version_id, matching the text-rects reader).
        if storage
            .load_geometry_page(r.user_id, r.blob_version_id, 1)
            .await
            .is_ok()
        {
            continue;
        }
        let Some(mime) = r.mime_type.as_deref() else {
            continue;
        };
        let ext = extension_of(&r.filename);
        let bytes = match storage
            .load_original(r.user_id, r.blob_version_id, &ext)
            .await
        {
            Ok(b) => b,
            Err(_) => continue,
        };
        let geometry = pm.geometry_pages(&bytes, mime).await;
        if geometry.is_empty() {
            continue;
        }
        for (i, g) in geometry.iter().enumerate() {
            let _ = storage
                .save_geometry_page(r.user_id, r.blob_version_id, (i as u32) + 1, g)
                .await;
        }
        done += 1;
    }

    if done > 0 {
        tracing::info!("geometry.backfill: captured geometry for {done} file(s)");
    }
}
