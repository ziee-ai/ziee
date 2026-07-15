//! Periodic retention prune for `notifications`.
//!
//! Spawned fire-and-forget at notification module init (mirrors
//! `mcp/tool_calls/prune.rs`). Reads the admin-configured retention window from
//! `scheduler_admin_settings` each tick; `0` days means keep forever.

use std::time::Duration;

use sqlx::PgPool;

/// How often the prune loop runs. Day-granularity retention needs no tighter
/// cadence; a fresh boot also runs one prune immediately.
const PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Run an initial prune, then prune every `PRUNE_INTERVAL` for the life of the
/// process. Never returns; intended to be `tokio::spawn`ed.
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
            tracing::warn!(error = %e, "notification: prune skipped (failed to read retention)");
            return;
        }
    };

    if days <= 0 {
        return; // keep forever
    }

    // Delegate to the SDK crate's retention prune (it takes the window in days
    // and computes the cutoff internally; also a no-op for days <= 0).
    match ziee_notification::repository::prune_older_than(pool, days as i64).await {
        Ok(0) => {}
        Ok(n) => tracing::info!("notification: pruned {n} rows older than {days} days"),
        Err(e) => tracing::warn!(error = %e, "notification: prune failed"),
    }
}

/// Read the deployment-wide notification retention window (days) from the
/// scheduler admin-settings singleton.
async fn retention_days(pool: &PgPool) -> Result<i32, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT notification_retention_days FROM scheduler_admin_settings WHERE id = TRUE"#,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.notification_retention_days)
}
