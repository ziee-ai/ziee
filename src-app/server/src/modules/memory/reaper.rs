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
