//! Periodic memory-retention reaper.
//!
//! Runs every 24 hours (configurable in code) and:
//!   1. Hard-deletes rows where `deleted_at < NOW() - INTERVAL '30 days'`
//!      (soft-deletes get a 30-day grace period for user audit).
//!   2. Enforces per-user `max_memories` by soft-deleting the oldest
//!      `updated_at` rows when the live count exceeds the cap.
//!   3. Enforces `retention_days` per user — soft-deletes rows where
//!      `updated_at < NOW() - retention_days days`.

use sqlx::PgPool;
use std::time::Duration;

const TICK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const SOFT_DELETE_GRACE_DAYS: i32 = 30;

/// Spawned at module init by `MemoryModule::init`.
pub async fn run_reaper_loop(pool: PgPool) {
    tracing::info!(
        "memory.reaper: started; tick={}s, soft-delete grace={}d",
        TICK_INTERVAL.as_secs(),
        SOFT_DELETE_GRACE_DAYS
    );
    loop {
        if let Err(e) = run_once(&pool).await {
            tracing::warn!("memory.reaper: tick failed: {e}");
        }
        tokio::time::sleep(TICK_INTERVAL).await;
    }
}

async fn run_once(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 1. Hard-delete grace-period-expired soft-deletes.
    let hard_deleted = sqlx::query(
        "DELETE FROM user_memories WHERE deleted_at IS NOT NULL AND deleted_at < NOW() - ($1 * INTERVAL '1 day')",
    )
    .bind(SOFT_DELETE_GRACE_DAYS)
    .execute(pool)
    .await?;
    if hard_deleted.rows_affected() > 0 {
        tracing::info!(
            "memory.reaper: hard-deleted {} grace-period-expired rows",
            hard_deleted.rows_affected()
        );
    }

    // 2. Per-user retention_days enforcement.
    let retention_deleted = sqlx::query(
        r#"
        UPDATE user_memories um
        SET deleted_at = NOW()
        FROM user_memory_settings ums
        WHERE um.user_id = ums.user_id
          AND ums.retention_days IS NOT NULL
          AND um.deleted_at IS NULL
          AND um.updated_at < NOW() - (ums.retention_days * INTERVAL '1 day')
        "#,
    )
    .execute(pool)
    .await?;
    if retention_deleted.rows_affected() > 0 {
        tracing::info!(
            "memory.reaper: soft-deleted {} retention-aged rows",
            retention_deleted.rows_affected()
        );
    }

    // 3. Per-user max_memories cap. For each user over the cap,
    // soft-delete the oldest `updated_at` rows down to the cap.
    //
    // Uses a window function so this is one round-trip per global
    // sweep instead of one per user.
    let cap_deleted = sqlx::query(
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
        "#,
    )
    .execute(pool)
    .await?;
    if cap_deleted.rows_affected() > 0 {
        tracing::info!(
            "memory.reaper: soft-deleted {} over-cap rows",
            cap_deleted.rows_affected()
        );
    }

    Ok(())
}
