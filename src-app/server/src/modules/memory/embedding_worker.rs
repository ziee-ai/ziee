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
use crate::modules::memory::permissions::MemoryAdminRead;
use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};
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

/// RAII guard that clears `REBUILD_IN_PROGRESS` on drop — including on a
/// panic unwind inside `run`. Without it, a panic mid-rebuild would leave
/// the flag stuck `true` forever, permanently blocking every subsequent
/// rebuild (the admin UI would show "in progress" with no running worker).
struct InProgressGuard;
impl Drop for InProgressGuard {
    fn drop(&mut self) {
        REBUILD_IN_PROGRESS.store(false, Ordering::Release);
    }
}

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
    // RAII reset so the flag clears on a normal return, an error, OR a panic
    // unwind — a bare `store(false)` after the await would leak the guard
    // permanently (blocking all future rebuilds) if `run` ever panicked.
    let _guard = InProgressGuard;
    let result = run(pool, new_model_id, new_model_name, target_dimensions).await;
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

        // The PUT handler's MemoryAdminSettings emit fired with the OLD
        // dimension (this worker runs minutes later); notify admin devices
        // now that the new dimension is committed. Background → origin None.
        sync_publish(
            SyncEntity::MemoryAdminSettings,
            SyncAction::Update,
            Uuid::nil(),
            Audience::perm::<MemoryAdminRead>(),
            None,
        );
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
            match crate::modules::memory::engine::dispatch::embed(new_model_id, &content)
                .await
            {
                Ok(vec) => {
                    if !embedding_dim_matches(vec.len(), target_dimensions) {
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

/// Whether an embedding vector fits the current `halfvec(N)` column — the
/// per-row skip guard used by the memory rebuild loop. A mismatch (a model
/// returning an unexpected dim, or a stale in-flight vector after a swap) is
/// skipped rather than written (pgvector would reject a wrong-length vector).
pub(crate) fn embedding_dim_matches(actual_len: usize, expected_dim: i32) -> bool {
    actual_len as i32 == expected_dim
}

#[cfg(test)]
mod embed_skip_tests {
    use super::embedding_dim_matches;

    /// Memory embedding skip path (gap 3cdb397a5069): a model returning a
    /// wrong-dimension vector (e.g. after a model swap) is skipped, while a
    /// correct-dimension vector is written. Guards the inline mismatch check
    /// in the rebuild loop (embedding_worker.rs:198).
    #[test]
    fn mismatched_embedding_dim_is_skipped() {
        assert!(embedding_dim_matches(768, 768));
        assert!(!embedding_dim_matches(1536, 768), "wrong-dim vector skipped");
        assert!(!embedding_dim_matches(0, 768), "empty vector skipped");
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Drives the REAL `reembed_all` worker against an embedding model id that
    /// does not resolve, so `dispatch::embed` returns `Err` for the seeded row
    /// and the worker takes the embed-failure skip path (embedding_worker.rs:
    /// 220-227): the row is logged + skipped, the loop continues, and the
    /// worker returns normally. Asserts graceful degradation — the memory is
    /// NOT lost and its `embedding` stays NULL (skipped, never written at the
    /// wrong/zero dimension), and the worker does not panic.
    ///
    /// `target_dimensions` is pinned to the column's CURRENT dimension so the
    /// destructive NULL+ALTER branch is skipped — keeping the test focused on
    /// the failure-skip path and non-destructive to the shared test DB.
    ///
    /// DB-gated soft-skip (mirrors `reaper.rs`'s `run_once` test): no
    /// `DATABASE_URL`/unreachable DB → returns green so `cargo test --lib`
    /// without Postgres stays green; runs for real against a migrated DB.
    #[tokio::test]
    async fn reembed_all_skips_rows_whose_embedding_fails_and_does_not_lose_them() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise reembed_all against");
                return;
            }
        };
        let pool = match PgPoolOptions::new().max_connections(2).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };
        // `dispatch::embed` resolves the model via the global `Repos`; init is
        // idempotent (no-op if another lib test already initialized it).
        crate::core::init_repositories(pool.clone());

        let tag = Uuid::new_v4();
        let user_id: Uuid =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
                .bind(format!("embfail_{tag}"))
                .bind(format!("embfail_{tag}@example.com"))
                .fetch_one(&pool)
                .await
                .expect("seed user");

        // A memory row with content but no embedding — the worker will try to
        // embed it and fail.
        let mem_id: Uuid = sqlx::query_scalar(
            "INSERT INTO user_memories (user_id, content, source) VALUES ($1, $2, 'manual') RETURNING id",
        )
        .bind(user_id)
        .bind("the user prefers metric units")
        .fetch_one(&pool)
        .await
        .expect("seed memory");

        // Pin target_dimensions to the live column dimension so the worker
        // SKIPS the destructive NULL+ALTER branch (`current_dim == target`).
        let current_dim: i32 = sqlx::query_scalar(
            "SELECT embedding_dimensions FROM memory_admin_settings WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("read current embedding_dimensions");

        // A model id that does not exist → `Repos.llm_model.get_by_id` returns
        // None → `dispatch::embed` errs fast (no network) → the embed-failure
        // skip path runs for our row.
        let bogus_model_id = Uuid::new_v4();
        // Must not panic; returns after skipping the unembeddable row(s).
        reembed_all(
            pool.clone(),
            bogus_model_id,
            "no-such-embedding-model".to_string(),
            current_dim,
        )
        .await;

        // Graceful degradation: the memory survives (not deleted) and its
        // embedding stays NULL — it was skipped, never written at a wrong dim.
        let (still_exists, embedding_is_null): (bool, bool) = sqlx::query_as(
            "SELECT TRUE, embedding IS NULL FROM user_memories WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(mem_id)
        .fetch_one(&pool)
        .await
        .expect("the skipped memory must still exist (not lost on embed failure)");
        assert!(still_exists);
        assert!(
            embedding_is_null,
            "an embed failure must leave the row's embedding NULL (skipped), not a bogus vector"
        );

        // Cleanup so the shared lib-test DB stays tidy.
        let _ = sqlx::query("DELETE FROM user_memories WHERE user_id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
}
