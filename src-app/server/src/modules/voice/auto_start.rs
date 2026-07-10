//! Lazy single-flight auto-start for THE single managed whisper-server.
//!
//! The first transcribe request drives the spawn; concurrent requests serialize
//! on `START_LOCK` and then take the fast path (a healthy, correctly-modelled
//! instance is already running). Ported from `llm_local_runtime::auto_start`,
//! collapsed to one instance:
//!
//!   - resolves the whisper binary via
//!     [`binary_manager::ensure_binary_path`](crate::modules::voice::binary_manager);
//!   - resolves the ggml model via [`model::ensure_model`](crate::modules::voice::model);
//!   - reads settings via `crate::core::Repos.voice.get_settings()`;
//!   - spawns, polls `/` health up to `auto_start_timeout_secs`, verifies the
//!     loopback bind, and persists the singleton `voice_runtime_instance` row;
//!   - guards restarts with a [`HealthStateMachine`] (exponential backoff + a
//!     5-crash/60s flap cap → `Failed`);
//!   - restarts (drain + respawn) when the configured model differs from the
//!     running instance's `active_model`.

use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::voice::deployment::get_deployment_manager;
use crate::modules::voice::engine::health::{
    HealthEvent, HealthStateMachine, InstanceState, Transition,
};

/// Flap-window crash cap is the primary give-up trigger; this bounds the
/// explicit restart-attempt counter as a backstop.
const MAX_RESTART_ATTEMPTS: u32 = 5;

/// A live handle to the running whisper-server — what the transcribe handler
/// uses to reach the loopback base URL.
#[derive(Debug, Clone)]
pub struct InstanceHandle {
    pub base_url: String,
}

/// Serializes the slow (spawn) path so two concurrent callers never fire two
/// spawns. The fast path (already-running) does not take this lock.
static START_LOCK: Mutex<()> = Mutex::const_new(());

/// The single instance's health state machine (restored from the persisted row
/// on first use). `None` until first touched.
static HEALTH: Mutex<Option<HealthStateMachine>> = Mutex::const_new(None);

/// In-memory last-used epoch-millis. `touch_last_used` writes it (hot-path
/// cheap); the reaper flushes it to `voice_runtime_instance.last_used_at`.
/// `< 0` = unset.
static LAST_USED_MS: AtomicI64 = AtomicI64::new(-1);

/// In-flight transcription count, for cooperative drain. The transcribe handler
/// holds an [`InflightGuard`] for the duration of a transcription.
static INFLIGHT: AtomicUsize = AtomicUsize::new(0);

// ───────────────────────────── state machine ────────────────────────────────

/// Seed the in-memory state machine from the persisted singleton row, once.
/// Lets the flap / give-up history survive a server restart.
pub async fn ensure_restored() {
    if HEALTH.lock().await.is_some() {
        return;
    }
    let restored = match load_instance_row().await {
        Ok(Some(row)) => HealthStateMachine::from_persisted(
            MAX_RESTART_ATTEMPTS,
            &row.state,
            row.restart_attempts,
            row.last_failure_reason,
        ),
        _ => HealthStateMachine::new(MAX_RESTART_ATTEMPTS),
    };
    let mut guard = HEALTH.lock().await;
    if guard.is_none() {
        *guard = Some(restored);
    }
}

async fn note_event(event: HealthEvent) -> Transition {
    ensure_restored().await;
    let mut guard = HEALTH.lock().await;
    guard
        .get_or_insert_with(|| HealthStateMachine::new(MAX_RESTART_ATTEMPTS))
        .on_event(event)
}

/// Move the in-memory state machine into `Starting` before a spawn attempt.
///
/// The singleton row defaults to `state = 'stopped'`, so a freshly-restored
/// machine sits in the all-absorbing `Stopped` state where `StartedOk` /
/// `Crashed` are both `NoOp` — the machine would never reach `Healthy` and the
/// crash-loop give-up could never fire. Calling this at the top of `do_start`
/// leaves `Stopped` so the subsequent `StartedOk` → `Healthy` and any later
/// `Crashed` advances the flap window normally. A `Failed` machine is left
/// alone (auto-start already gates on `is_failed()` before reaching here).
async fn mark_starting() {
    ensure_restored().await;
    let mut guard = HEALTH.lock().await;
    guard
        .get_or_insert_with(|| HealthStateMachine::new(MAX_RESTART_ATTEMPTS))
        .mark_starting();
}

/// Feed a detected runtime process-death into the state machine (the reaper's
/// liveness pass found the child exited). Returns `Some(new_state_name)` when
/// the crash drove a transition (so the caller persists the new `state`), and
/// persists the `failed` row itself when the crash tripped give-up. `None` when
/// the machine absorbed the event (already terminal).
pub async fn report_crashed() -> Option<String> {
    match note_event(HealthEvent::Crashed(None)).await {
        Transition::GiveUp { reason } => {
            persist_failed(&reason).await;
            Some("failed".to_string())
        }
        Transition::Restart { .. } => Some("restarting".to_string()),
        Transition::StateChanged { to, .. } => Some(to),
        Transition::NoOp => None,
    }
}

async fn is_failed() -> bool {
    ensure_restored().await;
    matches!(
        HEALTH.lock().await.as_ref().map(|sm| &sm.state),
        Some(InstanceState::Failed { .. })
    )
}

/// Feed a periodic health-probe result into the state machine (the reaper's
/// health-monitor input). Returns `Some((old, new))` when the probe drove a
/// transition, so the caller can persist the new `state` string.
pub async fn report_health(healthy: bool, reason: &str) -> Option<(String, String)> {
    let event = if healthy {
        HealthEvent::Ok
    } else {
        HealthEvent::Unhealthy(reason.to_string())
    };
    match note_event(event).await {
        Transition::StateChanged { from, to } => Some((from, to)),
        _ => None,
    }
}

/// Clear a `Failed` state so auto-start will retry. Driven by the admin
/// `/voice/instance/restart` path. Returns `true` when the instance was
/// actually `Failed`.
pub async fn clear_failed() -> bool {
    ensure_restored().await;
    let mut guard = HEALTH.lock().await;
    match guard.as_mut() {
        Some(sm) if matches!(sm.state, InstanceState::Failed { .. }) => {
            sm.on_event(HealthEvent::ClearFailed);
            true
        }
        _ => false,
    }
}

// ───────────────────────────── last-used / drain ────────────────────────────

/// Record that the instance was just used (auto-start itself is a use, and each
/// transcription touches it). In-memory only; flushed by the reaper.
pub fn touch_last_used() {
    LAST_USED_MS.store(chrono::Utc::now().timestamp_millis(), Ordering::Relaxed);
}

/// Drain the pending in-memory last-used timestamp (millis), if any.
pub fn take_pending_last_used() -> Option<i64> {
    let v = LAST_USED_MS.swap(-1, Ordering::Relaxed);
    if v < 0 { None } else { Some(v) }
}

/// Current number of in-flight transcriptions (drain gate).
pub fn inflight_count() -> usize {
    INFLIGHT.load(Ordering::Relaxed)
}

/// RAII guard held by the transcribe handler for the duration of a call, so the
/// reaper's drain can wait for outstanding transcriptions before SIGTERM.
pub struct InflightGuard(());

impl InflightGuard {
    pub fn acquire() -> Self {
        INFLIGHT.fetch_add(1, Ordering::Relaxed);
        touch_last_used();
        InflightGuard(())
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        INFLIGHT.fetch_sub(1, Ordering::Relaxed);
        touch_last_used();
    }
}

// ───────────────────────────── persistence ──────────────────────────────────

// Mirrors the persisted instance row. `ensure_restored` only consumes the
// health-state columns; the rest are kept for symmetry with the row + future
// restore logic.
struct InstanceRow {
    #[allow(dead_code)]
    active_model: Option<String>,
    #[allow(dead_code)]
    base_url: Option<String>,
    #[allow(dead_code)]
    status: String,
    state: String,
    restart_attempts: i32,
    last_failure_reason: Option<String>,
}

async fn load_instance_row() -> Result<Option<InstanceRow>, AppError> {
    let row = sqlx::query!(
        r#"SELECT active_model, base_url, status, state, restart_attempts, last_failure_reason
           FROM voice_runtime_instance WHERE id = TRUE"#,
    )
    .fetch_optional(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    Ok(row.map(|r| InstanceRow {
        active_model: r.active_model,
        base_url: r.base_url,
        status: r.status,
        state: r.state,
        restart_attempts: r.restart_attempts,
        last_failure_reason: r.last_failure_reason,
    }))
}

async fn persist_running(model: &str, port: u16, base_url: &str) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE voice_runtime_instance
           SET active_model = $1, local_port = $2, base_url = $3,
               status = 'running', state = 'healthy', state_changed_at = NOW(),
               last_used_at = NOW(), updated_at = NOW()
           WHERE id = TRUE"#,
        model,
        port as i32,
        base_url,
    )
    .execute(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

async fn persist_failed(reason: &str) {
    let _ = sqlx::query!(
        r#"UPDATE voice_runtime_instance
           SET status = 'stopped', state = 'failed', state_changed_at = NOW(),
               last_failure_reason = $1, updated_at = NOW()
           WHERE id = TRUE"#,
        reason,
    )
    .execute(Repos.pool())
    .await;
}

/// Mark the singleton row stopped with a specific fine `state` (e.g. `crashed`,
/// `stopped`). Best-effort.
pub async fn persist_stopped(state: &str) {
    let _ = sqlx::query!(
        r#"UPDATE voice_runtime_instance
           SET status = 'stopped', state = $1, state_changed_at = NOW(), updated_at = NOW()
           WHERE id = TRUE"#,
        state,
    )
    .execute(Repos.pool())
    .await;
}

// ───────────────────────────── ensure_running ───────────────────────────────

async fn get_settings() -> Result<crate::modules::voice::models::VoiceSettings, AppError> {
    Repos.voice.get_settings().await
}

fn not_ready(msg: impl Into<String>) -> AppError {
    // 503-mappable: the feature is on but the runtime isn't up yet / is busy.
    AppError::new(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "VOICE_RUNTIME_UNAVAILABLE",
        msg,
    )
}

/// Ensure the single whisper-server is running with the configured model and
/// return a handle to its loopback base URL.
///
/// Fast path: the persisted row says `running`, the desired model matches, and a
/// cheap `/` probe answers → return immediately. Slow path: take `START_LOCK`,
/// re-check, then (drain any stale/other-model instance and) spawn + poll health
/// + verify loopback + persist.
pub async fn ensure_running() -> Result<InstanceHandle, AppError> {
    let settings = get_settings().await?;
    if !settings.enabled {
        return Err(AppError::conflict("voice dictation is disabled"));
    }

    let dep = get_deployment_manager().local();
    let desired_model_file = crate::modules::voice::model::model_filename(&settings.model);

    // Fast path — healthy, correctly-modelled instance already running.
    if let Some(handle) = live_handle_if_current(&dep, &desired_model_file).await? {
        note_event(HealthEvent::StartedOk).await;
        touch_last_used();
        return Ok(handle);
    }

    // Don't auto-spawn an instance the flap cap has already failed.
    if is_failed().await {
        return Err(not_ready(
            "voice runtime is marked failed (flap protection); restart it before retrying",
        ));
    }

    // Slow path — serialize spawns.
    let _guard = START_LOCK.lock().await;

    // Double-check under the lock (a racing caller may have started it).
    if let Some(handle) = live_handle_if_current(&dep, &desired_model_file).await? {
        note_event(HealthEvent::StartedOk).await;
        touch_last_used();
        return Ok(handle);
    }

    match do_start(&settings).await {
        Ok(handle) => {
            note_event(HealthEvent::StartedOk).await;
            touch_last_used();
            Ok(handle)
        }
        Err(e) => {
            // A failed start counts as a crash for flap/backoff.
            if let Transition::GiveUp { reason } = note_event(HealthEvent::Crashed(None)).await {
                persist_failed(&reason).await;
            }
            Err(e)
        }
    }
}

/// Admin-initiated stop: SIGTERM the whisper-server, feed `AdminStop` to the
/// state machine, and persist the singleton row `stopped`. The next transcribe
/// (or an explicit restart) brings it back up.
pub async fn admin_stop() -> Result<(), AppError> {
    let _guard = START_LOCK.lock().await;
    get_deployment_manager().local().stop().await?;
    note_event(HealthEvent::AdminStop).await;
    persist_stopped("stopped").await;
    Ok(())
}

/// Admin-initiated restart: clear any `Failed` state, stop the running instance,
/// then bring it back up with the currently-configured model.
pub async fn admin_restart() -> Result<InstanceHandle, AppError> {
    clear_failed().await;
    {
        let _guard = START_LOCK.lock().await;
        let _ = get_deployment_manager().local().stop().await;
        persist_stopped("stopped").await;
    }
    ensure_running().await
}

/// Return a handle IFF the deployment currently runs a HEALTHY instance whose
/// model matches `desired_model_file`. Otherwise `None` (caller (re)starts).
async fn live_handle_if_current(
    dep: &crate::modules::voice::deployment::LocalDeployment,
    desired_model_file: &str,
) -> Result<Option<InstanceHandle>, AppError> {
    let status = dep.status().await;
    if !status.running {
        return Ok(None);
    }
    // Model change → not current; caller drains + restarts.
    match dep.active_model().await {
        Some(m) if m == desired_model_file => {}
        _ => return Ok(None),
    }
    let Some(port) = status.port else {
        return Ok(None);
    };
    let base_url = format!("http://127.0.0.1:{port}");
    if dep.health_check(&base_url).await.unwrap_or(false) {
        Ok(Some(InstanceHandle { base_url }))
    } else {
        Ok(None)
    }
}

/// Spawn + health-wait + loopback-verify + persist. The caller owns the
/// single-flight lock + the state-machine bookkeeping.
async fn do_start(
    settings: &crate::modules::voice::models::VoiceSettings,
) -> Result<InstanceHandle, AppError> {
    // Leave the absorbing `Stopped` state so `StartedOk`/`Crashed` below drive
    // real transitions (see `mark_starting`). Without this a persisted-`stopped`
    // machine never becomes `Healthy` and its crash-loop never trips give-up.
    mark_starting().await;

    let binary = crate::modules::voice::binary_manager::ensure_binary_path().await?;
    let model_path = crate::modules::voice::model::ensure_model(&settings.model).await?;

    let dep = get_deployment_manager().local();

    // Replace any stale / other-model instance (drain then respawn happens
    // inside `start`, which stops the previous process first).
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| AppError::internal_error("no available port for whisper-server"))?;

    let outcome = dep
        .start(&binary, &model_path, port, Some(&settings.language))
        .await?;

    // Poll `/` health until Ok or timeout.
    let timeout = Duration::from_secs(settings.auto_start_timeout_secs.max(1) as u64);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if dep.health_check(&outcome.base_url).await.unwrap_or(false) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = dep.stop().await;
            return Err(not_ready(format!(
                "whisper-server did not become healthy within {}s",
                timeout.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Confirm the listener is bound to loopback (defense-in-depth over --host).
    if !crate::modules::voice::deployment::LocalDeployment::verify_loopback_bind(
        outcome.pid,
        outcome.port,
    ) {
        let _ = dep.stop().await;
        return Err(AppError::internal_error(format!(
            "whisper-server bound a non-loopback address on port {} — refusing to register",
            outcome.port
        )));
    }

    let model_file = crate::modules::voice::model::model_filename(&settings.model);
    persist_running(&model_file, outcome.port, &outcome.base_url).await?;

    Ok(InstanceHandle {
        base_url: outcome.base_url,
    })
}
