//! Periodic retention prune for `scheduled_task_runs` (ITEM-8/DEC-7).
//!
//! Migration 144 documented run history as "time-pruned alongside notifications"
//! but nothing implemented it → unbounded growth for long-lived tasks. This boot-
//! spawned loop reuses the admin-configured `notification_retention_days` (0 =
//! keep forever), mirroring `notification/prune.rs`.

use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;

/// How often the prune runs. Day-granularity retention needs no tighter cadence;
/// a fresh boot also runs one prune immediately.
const PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Run an initial prune, then prune every `PRUNE_INTERVAL` for the process life.
/// Never returns; intended to be `tokio::spawn`ed.
pub async fn run_prune_loop(pool: PgPool) {
    loop {
        prune_once(&pool).await;
        tokio::time::sleep(PRUNE_INTERVAL).await;
    }
}

async fn prune_once(pool: &PgPool) {
    let days = match retention_days(pool).await {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "scheduler: run prune skipped (failed to read retention)");
            return;
        }
    };
    if days <= 0 {
        return; // keep forever
    }
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    match super::repository::prune_runs_older_than(pool, cutoff).await {
        Ok(0) => {}
        Ok(n) => tracing::info!("scheduler: pruned {n} run rows older than {days} days"),
        Err(e) => tracing::warn!(error = %e, "scheduler: run prune failed"),
    }
}

/// Deployment-wide retention window (days) from the scheduler admin singleton
/// (reused from notifications per DEC-7).
async fn retention_days(pool: &PgPool) -> Result<i32, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT notification_retention_days FROM scheduler_admin_settings WHERE id = TRUE"#,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.notification_retention_days)
}
