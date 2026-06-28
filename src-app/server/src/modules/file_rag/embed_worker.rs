//! Document-RAG embedding worker — re-embeds `file_chunks` when the admin sets
//! or changes the embedding model, performing the destructive
//! `ALTER COLUMN embedding TYPE halfvec(N)` migration when the new model has a
//! different dimension. A complete, independent copy of memory's worker
//! targeting `file_chunks` (Option B: file_rag owns its own embedder).
//!
//! Triggered (detached) from the admin PUT / `/reembed`. While a rebuild is in
//! flight, retrieval skips `embedding IS NULL` rows, so search degrades to
//! FTS-only without errors.

use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::memory::engine::dispatch::embed_batch;
use pgvector::HalfVector;

const REBUILD_BATCH_SIZE: i64 = 100;

/// Process-global guard so two concurrent admin PUTs can't interleave
/// NULL + ALTER + re-embed against the same `file_chunks.embedding` column.
static REBUILD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// RAII guard that clears `REBUILD_IN_PROGRESS` on drop — including on a
/// panic unwind inside `run`. Without it, a panic mid-rebuild would leave
/// the flag stuck `true` forever, permanently blocking every subsequent
/// rebuild.
struct InProgressGuard;
impl Drop for InProgressGuard {
    fn drop(&mut self) {
        REBUILD_IN_PROGRESS.store(false, Ordering::Release);
    }
}

/// True while a rebuild is in flight — surfaced to the admin UI so it can show
/// a progress banner instead of letting the operator trigger a second rebuild.
pub fn is_in_progress() -> bool {
    REBUILD_IN_PROGRESS.load(Ordering::Acquire)
}

/// Whether an embedding vector can be stored in the current `halfvec(N)`
/// column. After a model SWAP that changes the dimension, the column is ALTERed
/// to the new N first; any vector whose length still differs (e.g. a stale
/// in-flight batch, or a model that returns an unexpected dim) MUST be skipped
/// rather than written — a length mismatch would be rejected by pgvector. Pure
/// + shared by both the rebuild worker and the inline ingest path (ingest.rs).
pub(crate) fn embedding_dim_matches(actual_len: usize, expected_dim: i32) -> bool {
    actual_len as i32 == expected_dim
}

/// Re-embed all `file_chunks` with `model_id`. If `target_dimensions` differs
/// from the column's current dimension, first NULLs all embeddings, then
/// `ALTER`s the column + HNSW index. Tags rows with the model UUID so a model
/// swap (even at the same dimension) re-embeds the whole corpus.
pub async fn reembed_all(pool: PgPool, model_id: Uuid, target_dimensions: i32) {
    if REBUILD_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::warn!(
            "file_rag.embed_worker: a rebuild is already in progress; skipping concurrent run for model {model_id}"
        );
        return;
    }
    // Guard resets the flag on every exit path, including a panic unwind.
    let _guard = InProgressGuard;
    let result = run(pool, model_id, target_dimensions).await;
    if let Err(e) = result {
        tracing::warn!("file_rag.embed_worker: failed: {e}");
    }
}

async fn run(pool: PgPool, model_id: Uuid, target_dimensions: i32) -> Result<(), AppError> {
    let current_dim = sqlx::query_scalar!(
        r#"SELECT embedding_dimensions FROM file_rag_admin_settings WHERE id = 1"#
    )
    .fetch_one(&pool)
    .await
    .map_err(AppError::database_error)?;

    if current_dim != target_dimensions {
        tracing::info!(
            "file_rag.embed_worker: dimension change {current_dim} -> {target_dimensions} — NULLing + ALTER COLUMN"
        );
        // Make the whole reshape atomic: if any step (incl. the HNSW index
        // rebuild) fails, roll back so we never leave the column altered but
        // the settings row's recorded dimension stale.
        let mut tx = pool.begin().await.map_err(AppError::database_error)?;
        sqlx::query!("UPDATE file_chunks SET embedding = NULL, embedding_model = NULL")
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        sqlx::query!("DROP INDEX IF EXISTS idx_file_chunks_embedding")
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        // `target_dimensions` is an i32 from the controlled admin path (probe-
        // derived), interpolated into the TYPE because Postgres parses TYPE at
        // parse time, not as a bind parameter. No injection risk.
        let alter = format!(
            "ALTER TABLE file_chunks ALTER COLUMN embedding TYPE halfvec({target_dimensions})"
        );
        sqlx::query(&alter)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        sqlx::query!(
            "CREATE INDEX idx_file_chunks_embedding ON file_chunks USING hnsw (embedding halfvec_cosine_ops)"
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        sqlx::query!(
            "UPDATE file_rag_admin_settings SET embedding_dimensions = $1, updated_at = NOW() WHERE id = 1",
            target_dimensions
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        tx.commit().await.map_err(AppError::database_error)?;
    }

    // Re-embed every chunk whose embedding_model != model tag (or is NULL).
    let model_tag = model_id.to_string();
    let mut total: i64 = 0;
    loop {
        let batch = Repos
            .file_rag
            .chunks_needing_embedding(&model_tag, REBUILD_BATCH_SIZE)
            .await?;
        if batch.is_empty() {
            break;
        }
        let texts: Vec<String> = batch.iter().map(|(_, _, c)| c.clone()).collect();
        let vecs = match embed_batch(model_id, &texts).await {
            Ok(v) => v,
            Err(e) => {
                // Leave the rest NULL (FTS still works); admin can retry.
                tracing::warn!("file_rag.embed_worker: embed batch failed ({e}); aborting re-embed");
                break;
            }
        };
        let mut updated = 0usize;
        for ((id, uid, _), vec) in batch.iter().zip(vecs.iter()) {
            if !embedding_dim_matches(vec.len(), target_dimensions) {
            if !super::ingest::embedding_dim_ok(vec.len(), target_dimensions) {
                tracing::warn!(
                    "file_rag.embed_worker: model returned {}-dim vector but column is {}-dim — skipping chunk {}",
                    vec.len(),
                    target_dimensions,
                    id
                );
                continue;
            }
            let hv = HalfVector::from_f32_slice(vec);
            // Log-and-continue on a single bad write rather than aborting the
            // whole rebuild; the `updated == 0` guard still terminates a batch
            // that makes no progress.
            match Repos.file_rag.set_chunk_embedding(*id, *uid, &hv, &model_tag).await {
                Ok(()) => {
                    updated += 1;
                    total += 1;
                }
                Err(e) => {
                    tracing::warn!("file_rag.embed_worker: write embedding for {id} failed: {e}");
                }
            }
        }
        // No-progress guard: if a full non-empty batch produced zero updates
        // (persistent dimension mismatch), stop instead of spinning forever.
        if updated == 0 {
            tracing::warn!(
                "file_rag.embed_worker: no progress on a batch of {} (dimension mismatch?); stopping",
                batch.len()
            );
            break;
        }
    }
    tracing::info!("file_rag.embed_worker: re-embedded {total} chunks with model {model_id} (dim {target_dimensions})");
    Ok(())
}

#[cfg(test)]
mod dim_guard_tests {
    use super::embedding_dim_matches;

    /// Model-swap stale-chunk guard (gap f41785a24732): after swapping to a
    /// new embedding model the column is ALTERed to the new dimension; a vector
    /// whose length matches the (new) expected dim is written, one that doesn't
    /// (a stale old-dim vector, or a model returning an unexpected dim) is
    /// skipped. Used by both embed_worker (rebuild) and ingest (inline embed).
    #[test]
    fn matching_dim_is_writable_mismatch_is_skipped() {
        assert!(embedding_dim_matches(768, 768), "exact match writes");
        assert!(embedding_dim_matches(3072, 3072));
        // Stale chunk from the previous model (old dim) after a swap to 3072.
        assert!(!embedding_dim_matches(768, 3072), "old-dim vector must be skipped");
        // Model returns a shorter-than-expected vector.
        assert!(!embedding_dim_matches(512, 768));
        // Degenerate: empty vector is never writable.
        assert!(!embedding_dim_matches(0, 768));
mod tests {
    use super::*;

    /// All assertions live in ONE test fn because they mutate the
    /// process-global `REBUILD_IN_PROGRESS` static; splitting them into
    /// separate `#[test]` fns would let cargo's parallel runner interleave
    /// them and flake. No other test in the crate touches this static, so a
    /// single serial fn fully owns it. We assert the static is `false` on
    /// entry and restore it to `false` on exit so the suite is order-clean.
    #[test]
    fn rebuild_guard_single_flight_and_raii_clear() {
        // Precondition: nothing in flight at the start of the test.
        assert!(
            !is_in_progress(),
            "REBUILD_IN_PROGRESS must start cleared"
        );

        // --- RAII guard sets the flag on construction-equivalent and clears
        //     it on drop (the panic-safety contract reembed_all relies on). ---
        {
            // Mirror reembed_all's acquire: CAS false -> true must succeed.
            assert!(
                REBUILD_IN_PROGRESS
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok(),
                "first acquire of the rebuild flag must win the CAS"
            );
            let _guard = InProgressGuard;
            assert!(
                is_in_progress(),
                "is_in_progress() must report true while the flag is held"
            );

            // --- Single-flight: a concurrent acquire while held must LOSE the
            //     CAS (this is exactly the early-return path in reembed_all). ---
            assert!(
                REBUILD_IN_PROGRESS
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err(),
                "a second concurrent acquire must fail the CAS (single-flight)"
            );
            // Flag still held — the loser did not clobber it.
            assert!(is_in_progress());
        } // _guard drops here -> Drop clears the flag.

        // --- RAII-clears-on-drop: after the guard's scope, the flag is free. ---
        assert!(
            !is_in_progress(),
            "InProgressGuard::drop must clear REBUILD_IN_PROGRESS"
        );

        // --- A fresh acquire now succeeds again (the system recovered, not
        //     wedged true forever — the exact bug the guard prevents). ---
        assert!(
            REBUILD_IN_PROGRESS
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "after the guard drops, the next rebuild must be able to acquire"
        );
        // Restore the static so the rest of the suite sees a clean slate.
        REBUILD_IN_PROGRESS.store(false, Ordering::Release);
        assert!(!is_in_progress());
    }
}
