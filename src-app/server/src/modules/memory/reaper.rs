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
    let hard_deleted = sqlx::query!(
        "DELETE FROM user_memories WHERE deleted_at IS NOT NULL AND deleted_at < NOW() - ($1 * INTERVAL '1 day')",
        grace_days
    )
    .execute(pool)
    .await?;
    if hard_deleted.rows_affected() > 0 {
        tracing::info!(
            "memory.reaper: hard-deleted {} grace-period-expired rows",
            hard_deleted.rows_affected()
        );
    }

    // Owners whose VISIBLE list changed this sweep (a live row flipped to
    // soft-deleted in step 2 or 3). RETURNING the user_id lets us notify each
    // affected owner's other devices to reload.
    let mut touched_users: HashSet<Uuid> = HashSet::new();

    // 2. Per-user retention_days enforcement.
    let retention_rows = sqlx::query!(
        r#"
        UPDATE user_memories um
        SET deleted_at = NOW()
        FROM user_memory_settings ums
        WHERE um.user_id = ums.user_id
          AND ums.retention_days IS NOT NULL
          AND um.deleted_at IS NULL
          AND um.updated_at < NOW() - (ums.retention_days * INTERVAL '1 day')
        RETURNING um.user_id
        "#
    )
    .fetch_all(pool)
    .await?;
    if !retention_rows.is_empty() {
        tracing::info!(
            "memory.reaper: soft-deleted {} retention-aged rows",
            retention_rows.len()
        );
    }
    touched_users.extend(retention_rows.into_iter().map(|r| r.user_id));

    // 3. Per-user max_memories cap. Window function: one round-trip
    // per global sweep instead of one per user.
    let cap_rows = sqlx::query!(
        r#"
        WITH ranked AS (
            SELECT um.id,
                   ROW_NUMBER() OVER (PARTITION BY um.user_id ORDER BY um.updated_at DESC) AS rn,
                   COALESCE(ums.max_memories, 1000) AS cap
            FROM user_memories um
            LEFT JOIN user_memory_settings ums ON ums.user_id = um.user_id
            WHERE um.deleted_at IS NULL
        )
        UPDATE user_memories
        SET deleted_at = NOW()
        WHERE id IN (SELECT id FROM ranked WHERE rn > cap)
        RETURNING user_id
        "#
    )
    .fetch_all(pool)
    .await?;
    if !cap_rows.is_empty() {
        tracing::info!(
            "memory.reaper: soft-deleted {} over-cap rows",
            cap_rows.len()
        );
    }
    touched_users.extend(cap_rows.into_iter().map(|r| r.user_id));

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
