//! Document-RAG ingest — background chunk + embed over a file's extracted text.
//!
//! Triggered (non-blocking) from the `file` module's four head-change sites
//! (upload, files_mcp create_file, commit_new_version, restore_version). The
//! chunk insert is cheap and makes FTS work immediately; embedding runs after,
//! only when an embedding model is configured, and is best-effort (failure
//! leaves chunks unembedded → search degrades to FTS, like memory).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use once_cell::sync::Lazy;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::chunking::{ChunkParams, chunk_page};
use super::models::ChunkDraft;
use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::models::File;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::memory::engine::dispatch::embed_batch;
use pgvector::HalfVector;

/// Embedding batch size — matches memory's `REBUILD_BATCH_SIZE`.
const EMBED_BATCH_SIZE: usize = 100;
/// Per-page byte ceiling for chunking, to bound memory on a pathological
/// single-page file (e.g. a huge CSV/log). Truncated at a char boundary.
const MAX_PAGE_BYTES: usize = 5_000_000;
/// Files scanned per backfill batch.
const BACKFILL_BATCH: i64 = 200;

/// Truncate an over-large page in place to at most `MAX_PAGE_BYTES`, cutting on
/// a UTF-8 char boundary so the result is never invalid UTF-8 (a naive byte cut
/// could land mid-codepoint and panic on `String::truncate`). Returns whether a
/// truncation occurred.
fn truncate_to_max_page_bytes(text: &mut String) -> bool {
    if text.len() <= MAX_PAGE_BYTES {
        return false;
    }
    let mut cut = MAX_PAGE_BYTES;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
    true
}

/// Resolve the backfill batch size. A debug-only env override
/// (`FILE_RAG_BACKFILL_BATCH`, compiled out of release builds via
/// `cfg!(debug_assertions)` — same testability-seam pattern as
/// `LLM_RUNTIME_REAPER_TICK_MS`) lets the starvation integration test
/// reproduce the ≥batch wall-off with a handful of files instead of the
/// hundreds it would otherwise take. Ignored in release builds.
fn backfill_batch() -> i64 {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("FILE_RAG_BACKFILL_BATCH") {
        if let Ok(n) = v.parse::<i64>() {
            if n > 0 {
                return n;
            }
        }
    }
    BACKFILL_BATCH
}
/// Cap on concurrent per-file embed passes. A bulk upload of large files fans
/// out one detached index task per file; this bounds the *paid* embedding work
/// (and the load on the embedder) so it can't be amplified without limit.
const MAX_CONCURRENT_EMBED_JOBS: usize = 4;

static EMBED_CONCURRENCY: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(MAX_CONCURRENT_EMBED_JOBS));
/// Single-flight guard for `run_backfill` within this process (the boot
/// backfill + a manual `/backfill` could otherwise overlap and double-scan).
static BACKFILL_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Spawn a detached index task for a newly-created file (head = v1). No-op when
/// the file has no extracted text. Called right after `publish_file_changed`
/// at the upload + files_mcp create_file sites.
pub fn spawn_index(user_id: Uuid, file: &File) {
    if file.text_page_count <= 0 {
        return;
    }
    let (file_id, blob_version_id, version, pages) =
        (file.id, file.blob_version_id, file.version, file.text_page_count);
    tokio::spawn(async move {
        if let Err(e) = index_file_version(user_id, file_id, blob_version_id, version, pages).await
        {
            tracing::warn!("file_rag: index of {file_id} failed: {e}");
        }
    });
}

/// Spawn a detached re-index for a file whose head version changed (edit /
/// rewrite / sandbox version-back / restore). Called right after
/// `publish_file_changed` at the commit_new_version + restore_version sites.
pub fn spawn_reindex(user_id: Uuid, file_id: Uuid) {
    tokio::spawn(async move {
        if let Err(e) = reindex_file(user_id, file_id).await {
            tracing::warn!("file_rag: reindex of {file_id} failed: {e}");
        }
    });
}

/// Resolve the current head and (re)index it.
pub async fn reindex_file(user_id: Uuid, file_id: Uuid) -> Result<(), AppError> {
    let file = Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    index_file_version(
        user_id,
        file.id,
        file.blob_version_id,
        file.version,
        file.text_page_count,
    )
    .await
}

/// Chunk + (optionally) embed one file/version. The new chunk set is built
/// FIRST, then swapped in atomically via `reindex_chunks` (a per-file-serialized
/// transaction) — so retrieval keeps seeing the prior version's chunks during
/// the (slow) page-load + chunk, and a failed insert can't wipe the index.
pub async fn index_file_version(
    user_id: Uuid,
    file_id: Uuid,
    blob_version_id: Uuid,
    version: i32,
    text_page_count: i32,
) -> Result<(), AppError> {
    let admin = Repos.file_rag.get_admin_settings().await?;
    if !admin.enabled {
        // Disable is retrieval-only: ingest no-ops and existing chunks persist
        // (inert — `semantic_search` self-gates on `enabled` and returns the
        // disabled note). Re-enabling makes them live again; a chunking-param
        // change applies on each file's next re-index, not retroactively.
        return Ok(());
    }

    let storage = get_file_storage();
    let params = ChunkParams::from_settings(&admin);
    let max_chunks = admin.max_chunks_per_file.max(1) as usize;

    // Build the new chunks BEFORE touching the existing index (atomic swap below).
    let mut drafts: Vec<ChunkDraft> = Vec::new();
    let mut chunk_index: i32 = 0;
    'pages: for page in 1..=text_page_count {
        let mut text = match storage
            .load_text_page(user_id, blob_version_id, page as u32)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                // Tolerate a missing/unreadable page (mirrors files_mcp read).
                tracing::warn!("file_rag: page {page} of {file_id} unreadable: {e}; skipping");
                continue;
            }
        };
        if truncate_to_max_page_bytes(&mut text) {
            tracing::warn!(
                "file_rag: page {page} of {file_id} exceeds {MAX_PAGE_BYTES} bytes; truncated for indexing"
            );
        }

        let page_drafts = chunk_page(&text, page, chunk_index, &params);
        chunk_index += page_drafts.len() as i32;
        for d in page_drafts {
            if drafts.len() >= max_chunks {
                tracing::warn!(
                    "file_rag: {file_id} hit max_chunks_per_file ({max_chunks}); remaining text not indexed"
                );
                break 'pages;
            }
            drafts.push(d);
        }
    }

    // Atomic, per-file-serialized swap (DELETE + INSERT in one tx). An empty
    // `drafts` (no extractable text) clears the file's index.
    Repos
        .file_rag
        .reindex_chunks(file_id, user_id, blob_version_id, version, &drafts)
        .await?;

    if drafts.is_empty() {
        return Ok(());
    }

    // Vector arm: embed in the background only when a model is configured.
    // Best-effort — failure leaves chunks unembedded (FTS still works).
    if admin.semantic_enabled {
        if let Some(model_id) = admin.embedding_model_id {
            embed_file_chunks(file_id, model_id, admin.embedding_dimensions).await;
        }
    }
    Ok(())
}

/// Embed (or re-embed) every chunk of one file that lacks an embedding for the
/// configured model. `embedding_model` is tagged with the model UUID so a
/// model swap re-triggers embedding for the whole corpus.
async fn embed_file_chunks(file_id: Uuid, model_id: Uuid, expected_dim: i32) {
    // Belt-and-braces: skip while a deployment-wide rebuild is in flight. The
    // rebuild re-embeds every NULL/stale chunk (including these) at the settled
    // dimension, so embedding here would only race the ALTER and likely fail
    // the dimension check. The chunks stay FTS-searchable until the rebuild
    // fills them in.
    if super::embed_worker::is_in_progress() {
        return;
    }
    // Bound concurrent paid embedding work across all in-flight ingest tasks.
    let _permit = EMBED_CONCURRENCY
        .acquire()
        .await
        .expect("embed semaphore is never closed");

    let model_tag = model_id.to_string();
    let chunks = match Repos
        .file_rag
        .chunks_needing_embedding_for_file(file_id, &model_tag)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("file_rag: load chunks-needing-embedding for {file_id} failed: {e}");
            return;
        }
    };

    for batch in chunks.chunks(EMBED_BATCH_SIZE) {
        // Re-check before each batch: a rebuild may have started after our
        // initial check, in which case it now owns all embedding writes.
        if super::embed_worker::is_in_progress() {
            return;
        }
        let texts: Vec<String> = batch.iter().map(|(_, _, c)| c.clone()).collect();
        match embed_batch(model_id, &texts).await {
            Ok(vecs) => {
                for ((id, uid, _), vec) in batch.iter().zip(vecs.iter()) {
                    if !embedding_dim_ok(vec.len(), expected_dim) {
                        tracing::warn!(
                            "file_rag: model returned {}-dim vector but column is {}-dim — skipping chunk {}",
                            vec.len(),
                            expected_dim,
                            id
                        );
                        continue;
                    }
                    let hv = HalfVector::from_f32_slice(vec);
                    if let Err(e) = Repos
                        .file_rag
                        .set_chunk_embedding(*id, *uid, &hv, &model_tag)
                        .await
                    {
                        tracing::warn!("file_rag: set_chunk_embedding for {id} failed: {e}");
                    }
                }
            }
            Err(e) => {
                // Degrade to FTS-only; the rebuild/backfill embed pass retries.
                tracing::warn!("file_rag: embedding {file_id} failed ({e}); chunks remain FTS-only");
                return;
            }
        }
    }
}

/// Bounded, idempotent backfill: index pre-existing files that have extracted
/// text but no chunks yet. Safe to re-run every boot (self-heals failed
/// indexing). Terminates: each batch must include a not-yet-attempted file or
/// the loop stops (a file with no extractable text never gains chunks, so the
/// `seen` guard prevents an infinite re-scan). Single-flight within the process
/// (the boot backfill + a manual `/backfill` won't double-scan); across
/// replicas the per-file lock in `reindex_chunks` keeps concurrent indexing of
/// the same file from corrupting (worst case is redundant work).
pub async fn run_backfill() {
    if BACKFILL_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::info!("file_rag.backfill: already running; skipping concurrent run");
        return;
    }
    run_backfill_inner().await;
    BACKFILL_IN_PROGRESS.store(false, Ordering::Release);
}

async fn run_backfill_inner() {
    let admin = match Repos.file_rag.get_admin_settings().await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("file_rag.backfill: load settings failed: {e}");
            return;
        }
    };
    if !admin.enabled {
        return;
    }

    // `seen` holds every file we've attempted. We exclude it from the scan so
    // the window advances past files that have text pages but yield zero chunks
    // (scanned / whitespace-only PDFs) — those match the predicate forever and
    // would otherwise wall off newer indexable files behind them. The loop ends
    // when the scan returns nothing new (terminates: `seen` grows every
    // non-empty batch, the file set is finite).
    let batch = backfill_batch();
    let mut seen: HashSet<Uuid> = HashSet::new();
    loop {
        let exclude: Vec<Uuid> = seen.iter().copied().collect();
        let targets = match Repos
            .file_rag
            .files_with_text_missing_chunks(batch, &exclude)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("file_rag.backfill: scan failed: {e}");
                return;
            }
        };
        if targets.is_empty() {
            break;
        }
        for t in &targets {
            seen.insert(t.file_id);
            if let Err(e) = index_file_version(
                t.user_id,
                t.file_id,
                t.blob_version_id,
                t.version,
                t.text_page_count,
            )
            .await
            {
                tracing::warn!("file_rag.backfill: index {} failed: {e}", t.file_id);
            }
        }
    }
}

#[cfg(test)]
mod ingest_tests {
    use super::{truncate_to_max_page_bytes, MAX_PAGE_BYTES};

    #[test]
    fn under_cap_is_untouched() {
        let mut s = "short text".to_string();
        assert!(!truncate_to_max_page_bytes(&mut s));
        assert_eq!(s, "short text");
    }

    #[test]
    fn at_cap_is_untouched() {
        let mut s = "a".repeat(MAX_PAGE_BYTES);
        assert!(!truncate_to_max_page_bytes(&mut s));
        assert_eq!(s.len(), MAX_PAGE_BYTES);
    }

    #[test]
    fn oversized_ascii_truncates_to_cap() {
        let mut s = "a".repeat(MAX_PAGE_BYTES + 1234);
        assert!(truncate_to_max_page_bytes(&mut s));
        assert_eq!(s.len(), MAX_PAGE_BYTES, "ASCII cuts exactly at the cap");
    }

    #[test]
    fn oversized_multibyte_truncates_on_char_boundary_no_panic() {
        // '€' is 3 bytes; build a string whose byte length exceeds the cap and
        // whose codepoints do NOT line up with MAX_PAGE_BYTES, so a naive byte
        // cut would split a codepoint. The helper must back up to a boundary.
        let mut s = "€".repeat(MAX_PAGE_BYTES / 3 + 100);
        assert!(s.len() > MAX_PAGE_BYTES);
        assert!(truncate_to_max_page_bytes(&mut s));
        assert!(s.len() <= MAX_PAGE_BYTES, "never exceeds the cap");
        assert!(s.len() > MAX_PAGE_BYTES - 3, "backs up at most one codepoint");
        // The result is still valid UTF-8 (it is a String, so this also proves
        // no mid-codepoint truncation panicked).
        assert!(s.chars().all(|c| c == '€'));
    }
}

/// After an embedding-model swap, a re-embed pass can receive a vector whose
/// dimension no longer matches the `file_chunks.embedding` column (e.g. a 1024-d
/// model while the column is halfvec(768)). Storing it would error/corrupt the
/// index, so both `ingest` and `embed_worker` SKIP such chunks (left NULL →
/// FTS-only until a full column rebuild). This is the shared guard predicate.
pub(crate) fn embedding_dim_ok(vec_len: usize, expected_dim: i32) -> bool {
    vec_len as i32 == expected_dim
}

#[cfg(test)]
mod dim_guard_tests {
    use super::embedding_dim_ok;

    #[test]
    fn matching_dimension_is_stored_mismatch_is_skipped() {
        // Same dim → keep.
        assert!(embedding_dim_ok(768, 768));
        // Post-swap larger model → mismatch → skip (NOT stored as corrupt).
        assert!(!embedding_dim_ok(1024, 768));
        // Smaller / zero-length degenerate vectors are also skipped.
        assert!(!embedding_dim_ok(384, 768));
        assert!(!embedding_dim_ok(0, 768));
    }
}
