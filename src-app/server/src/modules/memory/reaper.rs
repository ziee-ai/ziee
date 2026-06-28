//! Periodic memory-retention reaper.
//!
//! Runs every 24 hours and:
//!   1. Hard-deletes rows where `deleted_at` is older than the
//!      admin-configured `soft_delete_grace_days` window (default 30d).
//!   2. Enforces per-user `max_memories` by soft-deleting the oldest
//!      `updated_at` rows when the live count exceeds the cap.
//!   3. Enforces `retention_days` per user — soft-deletes rows where
//!      `updated_at < NOW() - retention_days days`.
//!
//! The grace window is read fresh on every tick so admin changes via
//! `PUT /api/memory/admin-settings` take effect on the next sweep
//! without restarting the server.

use sqlx::PgPool;
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};

const TICK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Fallback grace days if the admin settings row can't be read (DB
/// transient error, fresh deployment racing). Matches the column
/// DEFAULT in migration 52.
const FALLBACK_GRACE_DAYS: f64 = 30.0;
/// How many rows each reaper sub-step touches per statement. The sweep
/// loops batch-by-batch so a large backlog never holds locks on the whole
/// `user_memories` table in a single long transaction (engineering chunk
/// size, not an operator-tunable policy).
const REAP_BATCH_SIZE: i64 = 5_000;

/// Spawned at module init by `MemoryModule::init`.
pub async fn run_reaper_loop(pool: PgPool) {
    tracing::info!(
        "memory.reaper: started; tick={}s, soft-delete grace read from admin settings each tick",
        TICK_INTERVAL.as_secs(),
    );
    loop {
        if let Err(e) = run_once(&pool).await {
            tracing::warn!("memory.reaper: tick failed: {e}");
        }
        tokio::time::sleep(TICK_INTERVAL).await;
    }
}

pub async fn run_once(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Read the live grace window. On error (transient DB blip), fall
    // back to the previous default so a sweep is never silently
    // skipped — better to delete on the conservative 30d window than
    // to leak stale soft-deletes indefinitely.
    let grace_days = match Repos.memory.get_admin_settings().await {
        Ok(s) => f64::from(s.soft_delete_grace_days),
        Err(e) => {
            tracing::warn!(
                "memory.reaper: get_admin_settings failed ({e}); falling back to {FALLBACK_GRACE_DAYS}d"
            );
            FALLBACK_GRACE_DAYS
        }
    };

    // 1. Hard-delete grace-period-expired soft-deletes. These rows are
    // ALREADY soft-deleted (gone from the owner's visible list), so purging
    // them changes nothing a client can see — no sync emit needed.
    let mut hard_deleted_total: u64 = 0;
    loop {
        let deleted = sqlx::query!(
            r#"
            DELETE FROM user_memories
            WHERE id IN (
                SELECT id FROM user_memories
                WHERE deleted_at IS NOT NULL
                  AND deleted_at < NOW() - ($1 * INTERVAL '1 day')
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            "#,
            grace_days,
            REAP_BATCH_SIZE,
        )
        .execute(pool)
        .await?;
        hard_deleted_total += deleted.rows_affected();
        if (deleted.rows_affected() as i64) < REAP_BATCH_SIZE {
            break;
        }
    }
    if hard_deleted_total > 0 {
        tracing::info!(
            "memory.reaper: hard-deleted {hard_deleted_total} grace-period-expired rows"
        );
    }

    // Owners whose VISIBLE list changed this sweep (a live row flipped to
    // soft-deleted in step 2 or 3). RETURNING the user_id lets us notify each
    // affected owner's other devices to reload.
    let mut touched_users: HashSet<Uuid> = HashSet::new();

    // 2. Per-user retention_days enforcement. Batched: each statement
    // locks + flips at most REAP_BATCH_SIZE rows (FOR UPDATE SKIP LOCKED on
    // the victim SELECT) so a large aged backlog can't hold a table-wide
    // lock for the whole sweep.
    let mut retention_total: usize = 0;
    loop {
        let retention_rows = sqlx::query!(
            r#"
            WITH victims AS (
                SELECT um.id
                FROM user_memories um
                JOIN user_memory_settings ums ON um.user_id = ums.user_id
                WHERE ums.retention_days IS NOT NULL
                  AND um.deleted_at IS NULL
                  AND um.updated_at < NOW() - (ums.retention_days * INTERVAL '1 day')
                LIMIT $1
                FOR UPDATE OF um SKIP LOCKED
            )
            UPDATE user_memories um
            SET deleted_at = NOW()
            FROM victims
            WHERE um.id = victims.id
            RETURNING um.user_id
            "#,
            REAP_BATCH_SIZE,
        )
        .fetch_all(pool)
        .await?;
        let n = retention_rows.len();
        retention_total += n;
        touched_users.extend(retention_rows.into_iter().map(|r| r.user_id));
        if (n as i64) < REAP_BATCH_SIZE {
            break;
        }
    }
    if retention_total > 0 {
        tracing::info!("memory.reaper: soft-deleted {retention_total} retention-aged rows");
    }

    // 3. Per-user max_memories cap. Window function: one round-trip
    // per global sweep instead of one per user.
    // Batched: each pass recomputes the per-user ranking over the still-live
    // rows and flips up to REAP_BATCH_SIZE of the over-cap victims. Because
    // each batch excludes rows already soft-deleted this sweep
    // (`deleted_at IS NULL`), the loop converges. (The victim CTE uses a
    // window function, so it can't take row locks — acceptable: the reaper
    // is the only writer of `deleted_at` for cap enforcement.)
    let mut cap_total: usize = 0;
    loop {
        let cap_rows = sqlx::query!(
            r#"
            WITH ranked AS (
                SELECT um.id,
                       ROW_NUMBER() OVER (PARTITION BY um.user_id ORDER BY um.updated_at DESC) AS rn,
                       COALESCE(ums.max_memories, 1000) AS cap
                FROM user_memories um
                LEFT JOIN user_memory_settings ums ON ums.user_id = um.user_id
                WHERE um.deleted_at IS NULL
            ),
            victims AS (
                SELECT id FROM ranked WHERE rn > cap LIMIT $1
            )
            UPDATE user_memories
            SET deleted_at = NOW()
            WHERE id IN (SELECT id FROM victims)
            RETURNING user_id
            "#,
            REAP_BATCH_SIZE,
        )
        .fetch_all(pool)
        .await?;
        let n = cap_rows.len();
        cap_total += n;
        touched_users.extend(cap_rows.into_iter().map(|r| r.user_id));
        if (n as i64) < REAP_BATCH_SIZE {
            break;
        }
    }
    if cap_total > 0 {
        tracing::info!("memory.reaper: soft-deleted {cap_total} over-cap rows");
    }

    // Notify each affected owner once. nil id = "the list changed, reload"
    // (same convention as delete_all_memories); background sweep → origin None.
    for user_id in touched_users {
        sync_publish(
            SyncEntity::Memory,
            SyncAction::Delete,
            Uuid::nil(),
            Audience::owner(user_id),
            None,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Drives the REAL single-pass `run_once` (the body of `run_reaper_loop`,
    /// which the existing `tests/memory/retention_test.rs` deliberately could
    /// not call — it re-implements the SQL because `ziee::modules` is private).
    /// Seeds the three states the reaper acts on and asserts each transition:
    ///   - a soft-deleted row past the grace window is HARD-deleted (gone),
    ///   - a live row older than the user's `retention_days` is SOFT-deleted,
    ///   - a fresh live row within retention survives untouched,
    ///   - and the per-user `max_memories` cap soft-deletes the oldest overflow.
    ///
    /// DB-gated: soft-skips (mirroring the suite's env-gated real-stack tests)
    /// when no Postgres is reachable, so `cargo test --lib` without a DB stays
    /// green; runs for real wherever `DATABASE_URL` points at a migrated DB.
    #[tokio::test]
    async fn run_once_reaps_grace_retention_and_cap() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise run_once against");
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
        // `run_once` reads admin settings via the global `Repos`; init is
        // idempotent (no-op if another lib test already won the race).
        crate::core::init_repositories(pool.clone());

        let tag = Uuid::new_v4();
        let user_id: Uuid =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
                .bind(format!("reaper_{tag}"))
                .bind(format!("reaper_{tag}@example.com"))
                .fetch_one(&pool)
                .await
                .expect("seed user");

        // retention_days = 1 (so a 2-day-old row is reaped), max_memories = 2.
        sqlx::query(
            "INSERT INTO user_memory_settings (user_id, retention_days, max_memories) \
             VALUES ($1, 1, 2)",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seed settings");

        // Helper: insert a memory row with explicit updated_at / deleted_at.
        async fn seed(
            pool: &PgPool,
            user_id: Uuid,
            content: &str,
            updated_age_days: f64,
            deleted_age_days: Option<f64>,
        ) -> Uuid {
            let deleted = deleted_age_days
                .map(|d| format!("NOW() - INTERVAL '{d} days'"))
                .unwrap_or_else(|| "NULL".to_string());
            let q = format!(
                "INSERT INTO user_memories (user_id, content, updated_at, created_at, deleted_at) \
                 VALUES ($1, $2, NOW() - INTERVAL '{updated_age_days} days', NOW(), {deleted}) \
                 RETURNING id"
            );
            sqlx::query_scalar(&q)
                .bind(user_id)
                .bind(content)
                .fetch_one(pool)
                .await
                .expect("seed memory")
        }

        // (1) grace: soft-deleted 40 days ago → hard-deleted (grace default 30d).
        let grace_expired = seed(&pool, user_id, "grace-expired", 40.0, Some(40.0)).await;
        // (1b) soft-deleted only 5 days ago → still within grace, NOT hard-deleted.
        let grace_fresh = seed(&pool, user_id, "grace-fresh", 5.0, Some(5.0)).await;
        // (2) retention: live, updated 2 days ago (> retention_days 1) → soft-deleted.
        let retention_aged = seed(&pool, user_id, "retention-aged", 2.0, None).await;
        // (3) fresh live row within retention → survives. Newest two updated_at
        // values among live rows so the cap keeps them.
        let fresh_a = seed(&pool, user_id, "fresh-a", 0.10, None).await;
        let fresh_b = seed(&pool, user_id, "fresh-b", 0.05, None).await;
        // (3b) cap: a 3rd live row. After retention_aged is reaped there are 3
        // live rows (retention_aged, fresh_a, fresh_b); retention drops the aged
        // one, leaving exactly 2 — at the cap. Add one more live row that is
        // within retention but OLDER than fresh_a/fresh_b so the cap evicts it.
        let cap_evicted = seed(&pool, user_id, "cap-evicted", 0.5, None).await;

        run_once(&pool)
            .await
            .expect("run_once completes without error");

        // Assert helper: read deleted_at; None row means hard-deleted.
        async fn state(pool: &PgPool, id: Uuid) -> Option<Option<chrono::DateTime<chrono::Utc>>> {
            sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
                "SELECT deleted_at FROM user_memories WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
            .expect("read state")
        }

        // (1) hard-deleted: the row is GONE entirely.
        assert!(
            state(&pool, grace_expired).await.is_none(),
            "grace-expired soft-deleted row must be hard-deleted (removed)"
        );
        // (1b) still soft-deleted but present (within grace).
        assert!(
            matches!(state(&pool, grace_fresh).await, Some(Some(_))),
            "within-grace soft-deleted row must survive the hard-delete"
        );
        // (2) retention-aged live row flipped to soft-deleted.
        assert!(
            matches!(state(&pool, retention_aged).await, Some(Some(_))),
            "retention-aged row must be soft-deleted"
        );
        // (3) fresh rows within retention + under cap survive (deleted_at NULL).
        assert!(
            matches!(state(&pool, fresh_a).await, Some(None)),
            "fresh-a within retention + under cap must survive"
        );
        assert!(
            matches!(state(&pool, fresh_b).await, Some(None)),
            "fresh-b within retention + under cap must survive"
        );
        // (3b) cap evicted the oldest over-cap live row.
        assert!(
            matches!(state(&pool, cap_evicted).await, Some(Some(_))),
            "over-cap oldest live row must be soft-deleted by the max_memories cap"
        );

        // Cleanup (best-effort) so reruns don't accumulate.
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
}
