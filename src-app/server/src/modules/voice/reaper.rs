//! Idle reaper + health monitor for the single managed whisper-server.
//!
//! Background loop, fixed 60s tick (debug override `WHISPER_RUNTIME_REAPER_TICK_MS`).
//! On each tick:
//!
//! 1. Flush the in-memory `last_used_at` touch to the singleton row (so the idle
//!    check sees current usage, not the frozen start time).
//! 2. Health-monitor pass: probe the running instance's `/` endpoint, feed the
//!    result into the state machine ([`auto_start::report_health`]), and persist
//!    any `state` transition. Only ever touches the `state` column — a degraded
//!    instance stays `running` (nothing is killed here).
//! 3. If `idle_unload_secs > 0` and the instance has been idle longer than that,
//!    drain (wait up to `drain_timeout_secs` for in-flight transcriptions) then
//!    SIGTERM and mark the row `stopped`.
//!
//! Ported from `llm_local_runtime::reaper`, collapsed to one instance.

use std::time::Duration;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::voice::auto_start;
use crate::modules::voice::deployment::get_deployment_manager;

const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Reaper tick cadence. Defaults to [`TICK_INTERVAL`] (60s). In **debug builds
/// only** `WHISPER_RUNTIME_REAPER_TICK_MS` may shorten it so the integration
/// suite can observe idle-eviction + drain behaviour in seconds. Compiled out of
/// release builds via `cfg!(debug_assertions)` — same testability-seam pattern as
/// `LLM_RUNTIME_REAPER_TICK_MS` / `CODE_SANDBOX_ROOTFS_MIRROR`.
fn tick_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("WHISPER_RUNTIME_REAPER_TICK_MS")
        && let Ok(ms) = ms.parse::<u64>()
        && ms > 0
    {
        return Duration::from_millis(ms);
    }
    TICK_INTERVAL
}

/// Spawn the background reaper task. Returns the JoinHandle so module init can
/// hold it if it ever wants graceful shutdown.
pub fn spawn() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_dur = tick_interval();
        tracing::info!(
            "voice::reaper: started (tick {}ms)",
            interval_dur.as_millis()
        );
        let mut interval = tokio::time::interval(interval_dur);
        loop {
            interval.tick().await;
            if let Err(e) = run_one_tick().await {
                tracing::warn!("voice::reaper tick failed: {e}");
            }
        }
    })
}

/// Body of one reaper tick. Public so tests can invoke it directly.
pub async fn run_one_tick() -> Result<(), AppError> {
    flush_last_used().await;

    let settings = Repos
        .voice
        .get_settings()
        .await
        .map_err(|e| AppError::internal_error(format!("reaper: settings load: {e}")))?;

    // Health-monitor pass runs regardless of whether idle eviction is enabled —
    // it only reads/updates the `state` column, never `status`.
    monitor_health().await;

    let idle_secs = settings.idle_unload_secs;
    if idle_secs <= 0 {
        return Ok(()); // eviction disabled
    }

    // Idle only if the instance is running AND last_used_at is older than the
    // threshold. `$1 * INTERVAL '1 second'` keeps the bind param int4-typed.
    let idle = sqlx::query_scalar!(
        r#"SELECT EXISTS(
             SELECT 1 FROM voice_runtime_instance
             WHERE id = TRUE AND status = 'running'
               AND last_used_at IS NOT NULL
               AND last_used_at < NOW() - ($1::int * INTERVAL '1 second')
           ) AS "exists!""#,
        idle_secs,
    )
    .fetch_one(Repos.pool())
    .await
    .map_err(|e| AppError::internal_error(format!("reaper: idle check: {e}")))?;

    if !idle {
        return Ok(());
    }

    if let Err(e) = drain_and_stop(settings.drain_timeout_secs).await {
        tracing::warn!("voice::reaper: drain_and_stop failed: {e}");
    }
    auto_start::persist_stopped("stopped").await;
    Ok(())
}

/// Probe the running instance and feed the result into the state machine;
/// persist any resulting `state` transition. Best-effort.
///
/// Detects an actual process EXIT (via `poll_liveness`) before the `/` probe: a
/// runtime crash must feed the state machine a `Crashed` event so the flap
/// window advances and a crash-loop trips give-up. A bare `/`-probe failure only
/// ever produced `Unhealthy`, which never advances the flap counter — so a
/// process that died at runtime would otherwise be respawned forever.
async fn monitor_health() {
    let dep = get_deployment_manager().local();
    let status = dep.status().await;
    let Some(port) = status.port else {
        return; // nothing running
    };
    if !status.running {
        return;
    }

    // A real process exit is a crash — advance the flap window, don't just mark
    // Unhealthy. `poll_liveness` reaps + clears the slot on exit.
    if dep.poll_liveness().await == Some(false) {
        if let Some(to) = auto_start::report_crashed().await {
            persist_health_state(&to).await;
        }
        return;
    }

    let base_url = format!("http://127.0.0.1:{port}");
    let healthy = dep.health_check(&base_url).await.unwrap_or(false);
    if let Some((from, to)) = auto_start::report_health(healthy, "whisper-server / probe failed").await
    {
        persist_health_state(&to).await;
        tracing::debug!("voice::reaper: health state {from} -> {to}");
    }
}

/// Persist the state-machine `state` name to the singleton row. Best-effort.
async fn persist_health_state(to: &str) {
    if let Err(e) = sqlx::query!(
        r#"UPDATE voice_runtime_instance
           SET state = $1, state_changed_at = NOW(), updated_at = NOW()
           WHERE id = TRUE"#,
        to,
    )
    .execute(Repos.pool())
    .await
    {
        tracing::warn!("voice::reaper: health state persist failed: {e}");
    }
}

/// Flush the in-memory last-used timestamp to the singleton row. Uses `NOW()`
/// (not the exact touch instant) — the tick is ≤60s and the idle threshold is
/// minutes, so the sub-tick precision loss is immaterial and it sidesteps a
/// chrono↔time bind-type conversion.
async fn flush_last_used() {
    if auto_start::take_pending_last_used().is_none() {
        return;
    }
    let _ = sqlx::query!(
        r#"UPDATE voice_runtime_instance
           SET last_used_at = NOW(), updated_at = NOW()
           WHERE id = TRUE AND status = 'running'"#,
    )
    .execute(Repos.pool())
    .await;
}

/// Wait up to `drain_timeout_secs` for in-flight transcriptions to finish, then
/// SIGTERM the whisper-server.
async fn drain_and_stop(drain_timeout_secs: i32) -> Result<(), AppError> {
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(drain_timeout_secs.max(1) as u64);
    loop {
        let inflight = auto_start::inflight_count();
        if inflight == 0 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!(
                "voice::reaper: drain timeout with {inflight} in-flight transcriptions; SIGTERM anyway"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    get_deployment_manager().local().stop().await
}
