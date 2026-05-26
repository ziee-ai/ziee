//! Embedding worker — re-embeds `user_memories` rows when the admin
//! changes the embedding model, and performs the destructive
//! `ALTER COLUMN embedding TYPE vector(N)` migration when the new
//! model has a different dimension.
//!
//! Triggered from the memory_admin update handler. Runs as a detached
//! `tokio::spawn` task so the admin's PUT returns immediately; the
//! actual rebuild can take minutes for large memory sets.
//!
//! While the rebuild is in flight, retrieval naturally skips rows
//! where `embedding IS NULL` (the existing query already filters),
//! so memory degrades gracefully without errors.

use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use crate::common::AppError;
use pgvector::HalfVector;

const REBUILD_BATCH_SIZE: i64 = 100;

/// Process-global in-progress flag. Closes audit finding R5-#1: two
/// concurrent admin PUTs to /api/memory/admin-settings each spawn a
/// worker; without this guard, both can interleave NULL+ALTER+re-embed
/// against the same `user_memories.embedding` column. The flag is
/// best-effort (process-local, not cluster-wide) — for single-server
/// deployments it's sufficient. Multi-process operators must rely on
/// admin-side discipline (don't change the embedding model from two
/// browsers at once).
static REBUILD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Re-embed all `user_memories` rows using `new_model_id`. If
/// `target_dimensions` differs from the column's current dimension,
/// first runs `ALTER TABLE user_memories ALTER COLUMN embedding TYPE
/// vector(N)` after NULLing all values, then re-embeds.
///
/// Caller is the admin update handler; on success the worker also
/// updates `memory_admin_settings.embedding_dimensions = target_dimensions`.
pub async fn reembed_all(
    pool: PgPool,
    new_model_id: Uuid,
    new_model_name: String,
    target_dimensions: i32,
) {
    // CAS-acquire the in-progress flag. If another worker holds it,
    // skip this run; the in-flight rebuild will eventually finish and
    // retrieval will see the new state once it does.
    if REBUILD_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::warn!(
            "memory.embedding_worker: another rebuild is in progress; skipping concurrent run for model {new_model_id}"
        );
        return;
    }
    let result = run(pool, new_model_id, new_model_name, target_dimensions).await;
    REBUILD_IN_PROGRESS.store(false, Ordering::Release);
    if let Err(e) = result {
        tracing::warn!("memory.embedding_worker: failed: {e}");
    }
}

/// True while a rebuild is in flight. Surfaced to admin UI so it can
/// show a progress banner instead of letting the operator trigger a
/// second rebuild that would no-op.
pub fn is_in_progress() -> bool {
    REBUILD_IN_PROGRESS.load(Ordering::Acquire)
}

async fn run(
    pool: PgPool,
    new_model_id: Uuid,
    new_model_name: String,
    target_dimensions: i32,
) -> Result<(), AppError> {
    // 1. Read current column dimension. If different, NULL + ALTER.
    let current_dim = sqlx::query_scalar!(
        r#"SELECT embedding_dimensions FROM memory_admin_settings WHERE id = 1"#
    )
    .fetch_one(&pool)
    .await
    .map_err(AppError::database_error)?;

    if current_dim != target_dimensions {
        tracing::info!(
            "memory.embedding_worker: dimension change {} -> {} — NULLing + ALTER COLUMN",
            current_dim,
            target_dimensions
        );

        // NULL all embeddings; embedding_model_id is locked-step.
        sqlx::query!("UPDATE user_memories SET embedding = NULL, embedding_model = NULL")
            .execute(&pool)
            .await
            .map_err(AppError::database_error)?;

        // ALTER COLUMN — must drop the hnsw index first (its
        // operator class is dimension-bound) and recreate after.
        // DROP/CREATE INDEX and ALTER COLUMN TYPE halfvec(N) use a
        // runtime-formatted string because `target_dimensions` becomes
        // part of the TYPE — Postgres parses TYPE at parse time, not
        // as a bind parameter. `target_dimensions` is an i32 from a
        // controlled admin path; no injection risk.
        sqlx::query!("DROP INDEX IF EXISTS idx_user_memories_embedding")
            .execute(&pool)
            .await
            .map_err(AppError::database_error)?;
        let alter = format!(
            "ALTER TABLE user_memories ALTER COLUMN embedding TYPE halfvec({})",
            target_dimensions
        );
        sqlx::query(&alter)
            .execute(&pool)
            .await
            .map_err(AppError::database_error)?;
        sqlx::query!(
            "CREATE INDEX idx_user_memories_embedding ON user_memories USING hnsw (embedding halfvec_cosine_ops)"
        )
        .execute(&pool)
        .await
        .map_err(AppError::database_error)?;

        // Commit the new dimension to admin settings so retrievers
        // pick it up. Done BEFORE re-embedding so a partial rebuild
        // leaves the row count consistent with the column type.
        sqlx::query!(
            "UPDATE memory_admin_settings SET embedding_dimensions = $1, updated_at = NOW() WHERE id = 1",
            target_dimensions
        )
        .execute(&pool)
        .await
        .map_err(AppError::database_error)?;
    }

    // 2. Re-embed every row whose embedding_model != new_model_name
    // (or is NULL). Batched to avoid loading huge memory lists into
    // process memory.
    let mut total_done: i64 = 0;
    loop {
        let batch = sqlx::query!(
            r#"
            SELECT id, user_id, content
            FROM user_memories
            WHERE deleted_at IS NULL
              AND (embedding IS NULL OR embedding_model IS DISTINCT FROM $1)
            LIMIT $2
            "#,
            new_model_name,
            REBUILD_BATCH_SIZE
        )
        .fetch_all(&pool)
        .await
        .map_err(AppError::database_error)?;

        if batch.is_empty() {
            break;
        }

        for row in batch {
            let id = row.id;
            let user_id = row.user_id;
            let content = row.content;
            match crate::modules::chat::extensions::memory::dispatch::embed(new_model_id, &content)
                .await
            {
                Ok(vec) => {
                    if vec.len() as i32 != target_dimensions {
                        tracing::warn!(
                            "memory.embedding_worker: model returned {}-dim vector but column is {}-dim — skipping row {}",
                            vec.len(),
                            target_dimensions,
                            id
                        );
                        continue;
                    }
                    let v = HalfVector::from_f32_slice(&vec);
                    let _ = sqlx::query(
                        "UPDATE user_memories SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4",
                    )
                    .bind(&v)
                    .bind(&new_model_name)
                    .bind(id)
                    .bind(user_id)
                    .execute(&pool)
                    .await
                    .map_err(AppError::database_error)?;
                    total_done += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "memory.embedding_worker: embed failed for row {}: {} — skipping",
                        id,
                        e
                    );
                }
            }
        }
    }

    tracing::info!(
        "memory.embedding_worker: rebuilt {} embeddings with model={} dim={}",
        total_done,
        new_model_name,
        target_dimensions
    );
    Ok(())
}
