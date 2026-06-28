//! Idle reaper for local engine instances.
//!
//! Background loop with a fixed 60s tick. On each tick:
//!
//! 1. Read `llm_runtime_settings.idle_unload_secs`. If 0, no-op.
//! 2. Read `llm_runtime_settings.drain_timeout_secs`.
//! 3. Find instances with `status = 'running'` AND
//!    `last_used_at < NOW() - idle_unload_secs * INTERVAL '1 sec'`.
//!    For each:
//!    a. Mark `proxy::InstanceFlag::Draining` (proxy front door 503s)
//!    b. Wait for `proxy::inflight_count(model_id)` to hit zero, OR
//!       `drain_timeout_secs` to elapse.
//!    c. Issue `LocalDeployment::stop` regardless (timeout chops
//!       mid-stream — see drain semantics in the plan).
//!    d. Clear in-memory flag + last-used + in-flight tracking.
//!    e. UPDATE the row to `status = 'stopped'`.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::types::Uuid;

use super::deployment::Deployment;
use super::proxy::{self, InstanceFlag};
use crate::common::AppError;
use crate::modules::llm_local_runtime::get_deployment_manager;

const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Reaper tick cadence. Defaults to [`TICK_INTERVAL`] (60s). In **debug
/// builds only** `LLM_RUNTIME_REAPER_TICK_MS` may shorten it so the
/// integration suite can observe idle-eviction + drain behaviour in
/// seconds instead of waiting a real minute. Compiled out of release
/// builds via `cfg!(debug_assertions)` — same testability-seam pattern
/// as `LLM_RUNTIME_RELEASE_MIRROR` and code_sandbox's mirror env.
fn tick_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("LLM_RUNTIME_REAPER_TICK_MS") {
        if let Ok(ms) = ms.parse::<u64>() {
            if ms > 0 {
                return Duration::from_millis(ms);
            }
        }
    }
    TICK_INTERVAL
}

/// Spawn the background reaper task. Returns the JoinHandle so the
/// module init can hold it for graceful shutdown if it ever wants to.
pub fn spawn(pool: Arc<PgPool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_dur = tick_interval();
        tracing::info!(
            "llm_local_runtime::reaper: started (tick {}ms)",
            interval_dur.as_millis()
        );
        let mut interval = tokio::time::interval(interval_dur);
        loop {
            interval.tick().await;
            if let Err(e) = run_one_tick(&pool).await {
                tracing::warn!("llm_local_runtime::reaper tick failed: {}", e);
            }
        }
    })
}

/// Body of one reaper tick. Public so it can be invoked directly
/// by tests.
pub async fn run_one_tick(pool: &PgPool) -> Result<(), AppError> {
    // Flush in-memory last_used_at touches to the DB BEFORE computing
    // idle candidates. `proxy::touch_last_used` writes in-memory only
    // (hot-path-cheap); without this flush the idle check below would
    // read the stale start-time value and evict actively-used engines.
    flush_last_used(pool).await;

    let settings = crate::core::repository::Repos
        .local_runtime
        .get_runtime_settings()
        .await
        .map_err(|e| AppError::internal_error(format!("reaper: settings load: {e}")))?;
    let (idle_secs, drain_secs) = (settings.idle_unload_secs, settings.drain_timeout_secs);

    if idle_secs <= 0 {
        // Eviction disabled.
        return Ok(());
    }

    // Find idle running instances. `$1 * INTERVAL '1 second'` keeps
    // the bind param int4-typed (vs `$1 || ' seconds'` which would
    // force a text param).
    let candidates = sqlx::query!(
        "SELECT model_id FROM llm_runtime_instances
         WHERE status = 'running'
           AND last_used_at < NOW() - ($1::int * INTERVAL '1 second')",
        idle_secs,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("reaper: candidates: {e}")))?;

    if candidates.is_empty() {
        return Ok(());
    }

    let dep = get_deployment_manager()
        .get_deployment(
            &crate::modules::llm_local_runtime::models::DeploymentConfig::Local {
                binary_path: None,
            },
        )
        .await?;

    for row in candidates {
        let model_id = row.model_id;
        // Cooperatively gate the proxy front door + drain in-flight.
        if let Err(e) = drain_and_stop(model_id, drain_secs, &dep).await {
            tracing::warn!(
                "reaper: drain_and_stop({model_id}) failed: {}",
                e
            );
        }

        // Mark the row stopped. Use `stopped` so /status surfaces the
        // transition.
        if let Err(e) = sqlx::query!(
            "UPDATE llm_runtime_instances
             SET status = 'stopped',
                 state  = 'stopped',
                 state_changed_at = NOW(),
                 stopped_at = NOW()
             WHERE model_id = $1",
            model_id,
        )
        .execute(pool)
        .await
        {
            tracing::warn!("reaper: row update for {model_id}: {}", e);
        }

        // Clear in-memory tracking so a future auto-start starts clean.
        // NOTE: we deliberately do NOT forget the in-flight counter
        // here — see proxy::forget_inflight's removal (H1/H2). The
        // counter is keyed by model_id and persists for the model's
        // lifetime; orphaning it while guards are live caused
        // premature SIGTERMs.
        proxy::clear_instance_flag(model_id).await;
        super::auto_start::forget(model_id).await;
    }

    Ok(())
}

/// Drain the in-memory last_used_at map and bump each touched
/// model's row to `NOW()`. Called at the top of every tick so the
/// idle check sees current usage rather than the frozen start-time
/// value. We use `NOW()` (not the exact in-memory touch timestamp)
/// because the reaper tick is ≤60s and the idle threshold is minutes,
/// so the sub-tick precision loss is immaterial — and it sidesteps a
/// chrono↔time bind-type conversion.
async fn flush_last_used(pool: &PgPool) {
    let model_ids: Vec<Uuid> = proxy::drain_last_used()
        .await
        .into_iter()
        .map(|(model_id, _ts)| model_id)
        .collect();
    if model_ids.is_empty() {
        return;
    }
    // One batched UPDATE for all touched models instead of a query per row.
    let _ = sqlx::query!(
        "UPDATE llm_runtime_instances SET last_used_at = NOW() WHERE model_id = ANY($1)",
        &model_ids,
    )
    .execute(pool)
    .await;
}

/// CAS the proxy flag to Draining; wait up to `drain_timeout_secs`
/// for the in-flight counter to reach 0; then SIGTERM the engine.
///
/// The proxy front door respects the Draining flag — new requests
/// get 503 + `retry_after_ms` — so the in-flight counter is bounded
/// by the time it takes existing streams to finish or hit timeout.
async fn drain_and_stop(
    model_id: Uuid,
    drain_timeout_secs: i32,
    dep: &Arc<dyn Deployment>,
) -> Result<(), AppError> {
    proxy::set_instance_flag(model_id, InstanceFlag::Draining).await;

    let deadline = tokio::time::Instant::now()
        + Duration::from_secs(drain_timeout_secs.max(1) as u64);
    loop {
        let inflight = proxy::inflight_count(model_id).await;
        if inflight == 0 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!(
                "reaper: drain timeout for {model_id} with {inflight} in-flight requests; SIGTERM anyway"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    // SIGTERM (LocalDeployment::stop already SIGKILLs after 10s if
    // the engine hasn't exited gracefully).
    dep.stop(model_id).await?;
    Ok(())
}
