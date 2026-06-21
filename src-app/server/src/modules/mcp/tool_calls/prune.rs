//! Periodic retention prune for `mcp_tool_calls`.
//!
//! Spawned fire-and-forget at MCP module init (mirrors the connection-health
//! loop). Reads the admin-configured retention window from `mcp_user_policy`
//! each tick; `0` days means keep forever.

use std::time::Duration;

use sqlx::PgPool;

use crate::core::Repos;
use crate::modules::mcp::user_policy;

/// How often the prune loop runs. Day-granularity retention doesn't need a
/// tighter cadence; a fresh boot also runs one prune immediately.
const PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Run an initial prune, then prune every `PRUNE_INTERVAL` for the life of the
/// process. Never returns; intended to be `tokio::spawn`ed.
pub async fn run_prune_loop(pool: PgPool) {
    loop {
        prune_once(&pool).await;
        tokio::time::sleep(PRUNE_INTERVAL).await;
    }
}

/// One prune pass: read the retention window, delete older rows.
async fn prune_once(pool: &PgPool) {
    let days = match user_policy::load(pool).await {
        Ok(policy) => policy.tool_call_retention_days,
        Err(e) => {
            tracing::warn!(error = %e, "mcp: tool-call prune skipped (failed to read retention)");
            return;
        }
    };

    if days <= 0 {
        return; // keep forever
    }

    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(days as i64);
    match Repos.mcp.prune_tool_calls(cutoff).await {
        Ok(0) => {}
        Ok(n) => tracing::info!("mcp: pruned {n} tool-call rows older than {days} days"),
        Err(e) => tracing::warn!(error = %e, "mcp: tool-call prune failed"),
    }
}
