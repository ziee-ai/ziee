//! Single-flight auto-start coordinator.
//!
//! Concurrent proxy requests for the same uncached model share one
//! `OnceCell<Result>` so we never fire two `LocalDeployment::start`
//! invocations for the same model_id. The first caller drives the
//! spawn; the rest await the OnceCell.
//!
//! On success the cell is removed from the map so a later failure
//! can be retried (e.g. operator manually starts after fixing a
//! file_path).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::sync::{Mutex, OnceCell};

use crate::common::AppError;
use crate::modules::llm_local_runtime::engine::{HealthEvent, HealthStateMachine, InstanceState, Transition};
use crate::modules::llm_local_runtime::get_deployment_manager;

/// Per-model single-flight start cell. The result string carries the
/// failure reason when an error is shared with concurrent waiters
/// (we can't share `AppError` across them — it's not Clone).
static IN_FLIGHT: LazyLock<Mutex<HashMap<Uuid, Arc<OnceCell<Result<(), String>>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-model health state machine (exponential backoff + flap
/// detection). The server has no periodic supervisor loop; instead we
/// consult/advance this lazily on each `ensure_running` request — a
/// crash-looping model lands in `Failed` (5 crashes / 60s) and stops
/// being re-spawned every request, instead of looping forever.
static HEALTH: LazyLock<Mutex<HashMap<Uuid, HealthStateMachine>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Flap-window crash cap is the primary give-up trigger; this bounds
/// the explicit restart-attempt counter as a backstop.
const MAX_RESTART_ATTEMPTS: u32 = 5;

/// Liveness of a model's instance, distinguishing a genuine crash (a
/// row that was `running` but the engine is dead) from "never started".
enum Liveness {
    Running,
    Crashed,
    NotRunning,
}

/// Seed the in-memory state machine for `model_id` from its persisted
/// instance row, once, if not already present. This is what lets the
/// flap/give-up history (`state` / `restart_attempts` / `last_failure_reason`)
/// survive a server restart — without it the `HEALTH` map starts empty on
/// boot and a model the flap cap had already marked `failed` would be
/// auto-respawned from a blank slate.
async fn ensure_restored(model_id: Uuid) {
    // Fast path: already in memory — never clobber live in-memory state.
    if HEALTH.lock().await.contains_key(&model_id) {
        return;
    }
    // Load the persisted row WITHOUT holding the HEALTH lock across the await.
    let restored = match crate::core::repository::Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await
    {
        Ok(Some(inst)) => HealthStateMachine::from_persisted(
            MAX_RESTART_ATTEMPTS,
            &inst.state,
            inst.restart_attempts,
            inst.last_failure_reason.clone(),
        ),
        // No row (model never started) or a transient DB error → fresh machine.
        _ => HealthStateMachine::new(MAX_RESTART_ATTEMPTS),
    };
    // Race-safe insert: keep whatever a concurrent task may have inserted.
    HEALTH.lock().await.entry(model_id).or_insert(restored);
}

/// Feed an event to the model's state machine; restore from persisted
/// state on first use so cross-restart recovery resumes correctly.
async fn note_event(model_id: Uuid, event: HealthEvent) -> Transition {
    ensure_restored(model_id).await;
    HEALTH
        .lock()
        .await
        .entry(model_id)
        .or_insert_with(|| HealthStateMachine::new(MAX_RESTART_ATTEMPTS))
        .on_event(event)
}

/// True once the state machine has given up on this model (flap cap).
/// Restores from persisted state first so a `failed` row from before a
/// restart still gates re-spawns.
async fn is_failed(model_id: Uuid) -> bool {
    ensure_restored(model_id).await;
    matches!(
        HEALTH.lock().await.get(&model_id).map(|sm| &sm.state),
        Some(InstanceState::Failed { .. })
    )
}

/// Best-effort persist of a `Failed` state to the instance row (UI +
/// proxy reflect it). No-op when no row exists (a model that never
/// successfully started) — the in-memory state still gates re-spawns.
async fn persist_failed(pool: &PgPool, model_id: Uuid, reason: &str) {
    let _ = sqlx::query!(
        "UPDATE llm_runtime_instances
         SET status = 'stopped', state = 'failed', state_changed_at = NOW(),
             last_failure_reason = $2, stopped_at = NOW()
         WHERE model_id = $1",
        model_id,
        reason,
    )
    .execute(pool)
    .await;
}

/// Look up the singleton runtime_settings.auto_start_timeout_secs via
/// the shared `Repos.local_runtime` repository.
async fn get_auto_start_timeout() -> Duration {
    let secs = crate::core::repository::Repos
        .local_runtime
        .get_runtime_settings()
        .await
        .map(|s| s.auto_start_timeout_secs)
        .unwrap_or(30);
    Duration::from_secs(secs.max(1) as u64)
}

/// Resolve a model's deployment inputs from the DB. Returns the
/// (engine_type, model_path, parameters) tuple `LocalDeployment::start`
/// expects.
///
/// Model file location, in priority order:
///  1. an explicit `llm_model_files` row (test seeding + any future
///     multi-file bookkeeping);
///  2. otherwise the on-disk storage convention every real ingest path
///     (download + upload) actually writes to:
///     `<app_data>/models/<provider_id>/<model_id>/`. Production never
///     populates `llm_model_files`, so this fallback is the canonical
///     resolution for downloaded/uploaded models.
///
/// A `.gguf` file is always preferred; a multi-file model (mistralrs
/// safetensors) resolves to its containing directory.
pub(crate) async fn resolve_model_inputs(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<(String, String, serde_json::Value), AppError> {
    // The engine knobs live in `engine_settings` (the consolidated
    // engine config), NOT `parameters`. `capabilities.text_embedding`
    // drives llama.cpp's `--embeddings` mode.
    let model = sqlx::query!(
        "SELECT engine_type, engine_settings, capabilities, provider_id \
         FROM llm_models WHERE id = $1",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::database_error(e))?
    .ok_or_else(|| AppError::not_found("model not found"))?;

    let files = sqlx::query!(
        "SELECT file_path FROM llm_model_files WHERE model_id = $1 ORDER BY uploaded_at",
        model_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::database_error(e))?;

    let model_path = if !files.is_empty() {
        // Prefer a .gguf file (deterministically — the first shard for a
        // multi-part GGUF); else fall back to the directory containing the
        // model files (multi-file safetensors → engine wants the dir).
        let ggufs: Vec<String> = files
            .iter()
            .map(|f| f.file_path.clone())
            .filter(|p| p.to_ascii_lowercase().ends_with(".gguf"))
            .collect();
        match prefer_gguf(ggufs) {
            Some(path) => path,
            None => std::path::Path::new(&files[0].file_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| files[0].file_path.clone()),
        }
    } else {
        resolve_model_file_on_disk(model.provider_id, model_id).ok_or_else(|| {
            AppError::internal_error("model has no files; cannot start engine")
        })?
    };

    let mut config = model
        .engine_settings
        .unwrap_or_else(|| serde_json::json!({}));
    let is_embedder = model
        .capabilities
        .as_ref()
        .and_then(|c| c.get("text_embedding"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_embedder {
        config["embeddings"] = serde_json::json!(true);
    }

    Ok((model.engine_type, model_path, config))
}

/// Pick the `.gguf` to hand the engine, deterministically. For a multi-part
/// GGUF the engine wants the first shard (`…-00001-of-…`); otherwise the
/// lexicographically-smallest, so the choice is stable regardless of
/// `read_dir` order or `llm_model_files` row order.
fn prefer_gguf(mut ggufs: Vec<String>) -> Option<String> {
    if ggufs.is_empty() {
        return None;
    }
    ggufs.sort();
    ggufs
        .iter()
        .find(|p| p.contains("-00001-of-"))
        .cloned()
        .or_else(|| ggufs.into_iter().next())
}

/// Resolve a model's file from the on-disk storage convention that the
/// download/upload paths write to: `<app_data>/models/<provider>/<model>/`.
///
/// Prefers a single `.gguf` file (handed to the engine directly); a
/// multi-file model (mistralrs safetensors set) resolves to the
/// containing directory. Returns `None` if the directory is missing or
/// empty.
fn resolve_model_file_on_disk(provider_id: Uuid, model_id: Uuid) -> Option<String> {
    let dir = crate::core::get_app_data_dir()
        .join("models")
        .join(provider_id.to_string())
        .join(model_id.to_string());

    let files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();

    if files.is_empty() {
        return None;
    }

    // Prefer a .gguf file (single-file engine input), deterministically.
    let ggufs: Vec<String> = files
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("gguf"))
                .unwrap_or(false)
        })
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    if let Some(gguf) = prefer_gguf(ggufs) {
        return Some(gguf);
    }

    // Multi-file (safetensors): hand the engine the containing directory.
    Some(dir.to_string_lossy().to_string())
}

/// Check whether the model already has a LIVE running instance.
///
/// A row with `status='running'` is necessary but not sufficient: the
/// engine could have crashed (or been killed out-of-band) leaving the
/// row stale (B2/B3). So we also do a cheap `/health` probe against
/// the recorded base_url. If the row says running but the engine
/// doesn't answer, we mark the row `stopped` and return false so the
/// caller (re)starts it — avoiding a forward to a dead port.
async fn probe_liveness(pool: &PgPool, model_id: Uuid) -> Result<Liveness, AppError> {
    let row = sqlx::query!(
        "SELECT base_url FROM llm_runtime_instances
         WHERE model_id = $1 AND status = 'running'",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::database_error(e))?;

    let Some(row) = row else {
        return Ok(Liveness::NotRunning);
    };
    let base_url = row.base_url;

    // Liveness probe via the deployment's health_check (cheap GET).
    let dep = get_deployment_manager()
        .get_deployment(
            &crate::modules::llm_local_runtime::models::DeploymentConfig::Local {
                binary_path: None,
            },
        )
        .await?;
    if dep.health_check(&base_url).await.unwrap_or(false) {
        return Ok(Liveness::Running);
    }

    // Stale row — engine crashed or was killed out-of-band. Mark it
    // stopped so the reaper/UI reflect reality and the caller restarts.
    tracing::warn!(
        "auto_start: model {model_id} row says running but {base_url} is unhealthy; \
         marking stopped and restarting"
    );
    let _ = sqlx::query!(
        "UPDATE llm_runtime_instances
         SET status = 'stopped', state = 'crashed', state_changed_at = NOW(), stopped_at = NOW()
         WHERE model_id = $1",
        model_id,
    )
    .execute(pool)
    .await;
    // Do NOT forget the in-flight counter here: a transient health
    // blip on the hot path must not reset accounting that live
    // InFlightGuards depend on (H1/H2). The counter persists for the
    // model's lifetime.
    Ok(Liveness::Crashed)
}

/// Persist a running instance row after a successful spawn so the
/// reaper + proxy lookup know about it.
async fn persist_instance(
    pool: &PgPool,
    model_id: Uuid,
    pid: i32,
    port: i32,
    base_url: &str,
) -> Result<(), AppError> {
    // Look up the provider_id from llm_models.
    let provider_id: Uuid = sqlx::query_scalar!(
        "SELECT provider_id FROM llm_models WHERE id = $1",
        model_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::database_error(e))?;

    let _ = pid; // pid isn't currently a column; reserve for future use
    sqlx::query!(
        "INSERT INTO llm_runtime_instances
            (model_id, provider_id, local_port, base_url, status, state, last_used_at)
         VALUES ($1, $2, $3, $4, 'running', 'healthy', NOW())
         ON CONFLICT (model_id) DO UPDATE SET
            local_port = EXCLUDED.local_port,
            base_url   = EXCLUDED.base_url,
            status     = 'running',
            state      = 'healthy',
            state_changed_at = NOW(),
            last_used_at     = NOW()",
        model_id,
        provider_id,
        port,
        base_url,
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::database_error(e))?;
    Ok(())
}

/// Ensure the engine for `model_id` is running. If a start is
/// already in flight, attach to it; otherwise initiate one.
///
/// - Fast path: instance row reports `status = 'running'` → return Ok immediately.
/// - Slow path: claim the per-model OnceCell; only the holder drives
///   the spawn. Other waiters get the same outcome.
/// - On any non-success outcome the cell is removed so a retry can
///   fire on the next request.
pub async fn ensure_running(pool: &PgPool, model_id: Uuid) -> Result<(), AppError> {
    // Consult liveness + the health state machine before (re)starting.
    match probe_liveness(pool, model_id).await? {
        Liveness::Running => {
            // Healthy — reset the state machine (clears any backoff).
            note_event(model_id, HealthEvent::StartedOk).await;
            return Ok(());
        }
        Liveness::Crashed => {
            // A previously-running engine is dead → advance the machine.
            match note_event(model_id, HealthEvent::Crashed(None)).await {
                Transition::GiveUp { reason } => {
                    persist_failed(pool, model_id, &reason).await;
                    forget(model_id).await;
                    return Err(AppError::internal_error(format!(
                        "engine for model {model_id} marked failed (flap protection): {reason}"
                    )));
                }
                Transition::Restart { next_at, .. }
                    if std::time::Instant::now() < next_at =>
                {
                    return Err(AppError::internal_error(
                        "engine is in restart backoff after a crash; retry shortly",
                    ));
                }
                _ => {} // backoff elapsed (or no-op) → proceed to restart
            }
        }
        Liveness::NotRunning => {
            // Don't auto-spawn a model the flap cap has already failed.
            if is_failed(model_id).await {
                return Err(AppError::internal_error(
                    "engine for this model is marked failed (flap protection); \
                     clear it before retrying",
                ));
            }
        }
    }

    // Claim or attach the OnceCell.
    let cell = {
        let mut map = IN_FLIGHT.lock().await;
        map.entry(model_id)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    };

    let timeout = get_auto_start_timeout().await;
    let pool_for_init = pool.clone();
    let model_id_for_init = model_id;

    // The init closure runs ONCE (the single-flight holder), so the
    // start outcome is recorded in the state machine exactly once even
    // when many waiters share the cell.
    let result = cell
        .get_or_init(|| async move {
            let r = do_start(&pool_for_init, model_id_for_init, timeout).await;
            match &r {
                Ok(()) => {
                    note_event(model_id_for_init, HealthEvent::StartedOk).await;
                }
                Err(_) => {
                    // A failed start counts as a crash for flap/backoff.
                    if let Transition::GiveUp { reason } =
                        note_event(model_id_for_init, HealthEvent::Crashed(None)).await
                    {
                        persist_failed(&pool_for_init, model_id_for_init, &reason).await;
                    }
                }
            }
            r.map_err(|e| format!("{e}"))
        })
        .await
        .clone();

    // Drop the cell so a future failure can retry.
    {
        let mut map = IN_FLIGHT.lock().await;
        map.remove(&model_id);
    }

    result.map_err(AppError::internal_error)
}

/// Do the actual spawn + health-wait. Separated so the caller above
/// only owns the single-flight bookkeeping.
async fn do_start(
    pool: &PgPool,
    model_id: Uuid,
    timeout: Duration,
) -> Result<(), AppError> {
    let (engine_type, file_path, config) = resolve_model_inputs(pool, model_id).await?;

    let dep_mgr = get_deployment_manager();
    let dep = dep_mgr
        .get_deployment(
            &crate::modules::llm_local_runtime::models::DeploymentConfig::Local {
                binary_path: None,
            },
        )
        .await?;

    let dr = match dep.start(model_id, &engine_type, &file_path, &config).await {
        Ok(dr) => dr,
        Err(e) => {
            // A concurrent caller may have started this model between
            // our fast-path liveness check and now (the single-flight
            // cell is removed after each completion, so a late waiter
            // can re-enter — M1). If the engine is in fact up, treat the
            // conflict as success rather than surfacing a spurious error.
            if matches!(
                probe_liveness(pool, model_id).await,
                Ok(Liveness::Running)
            ) {
                return Ok(());
            }
            return Err(e);
        }
    };

    // Poll /health until Ok or timeout.
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if dep.health_check(&dr.base_url).await.unwrap_or(false) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            // Best-effort: stop the engine to free resources.
            let _ = dep.stop(model_id).await;
            return Err(AppError::internal_error(format!(
                "auto_start: model {model_id} did not become Healthy within {}s",
                timeout.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // P1.g: confirm the engine is bound to loopback. The spawn args
    // already pass `--host 127.0.0.1` but a malicious or buggy engine
    // could ignore that. Refuse to register if the listening socket
    // is anywhere else — the proxy is the only thing on the box that
    // should be able to reach the engine.
    if !crate::modules::llm_local_runtime::deployment::local::LocalDeployment::verify_loopback_bind(
        dr.pid, dr.port,
    ) {
        let _ = dep.stop(model_id).await;
        return Err(AppError::internal_error(format!(
            "auto_start: model {model_id} engine bound non-loopback address on port {} — refusing to register",
            dr.port
        )));
    }

    persist_instance(pool, model_id, dr.pid, dr.port, &dr.base_url).await?;

    // Touch last_used_at right away — the auto-start is itself a use.
    crate::modules::llm_local_runtime::proxy::touch_last_used(model_id).await;
    Ok(())
}

/// Feed a periodic health-probe result into the model's state machine. This
/// is the "health loop" input: the reaper's `monitor_health` pass probes each
/// running engine's `/health` and calls this with the boolean outcome, turning
/// a bare liveness bool into the richer `Healthy`/`Unhealthy` states the UI +
/// recovery logic read. Returns `Some((old_state, new_state))` when the probe
/// drove a state transition (so the caller can persist the new `state` string),
/// else `None`.
pub async fn report_health(
    model_id: Uuid,
    healthy: bool,
    reason: &str,
) -> Option<(String, String)> {
    let event = if healthy {
        HealthEvent::Ok
    } else {
        HealthEvent::Unhealthy(reason.to_string())
    };
    match note_event(model_id, event).await {
        Transition::StateChanged { from, to } => Some((from, to)),
        _ => None,
    }
}

/// Clear a model's `Failed` state (flap / restart-cap give-up) so auto-start
/// will retry it. Driven by the admin `clear-failed` REST endpoint. Restores
/// from persisted state first so a `failed` row from before a server restart
/// still resolves. Returns `true` when the model was in fact `Failed` (so the
/// caller can decide whether to persist + emit a status-change event).
pub async fn clear_failed(model_id: Uuid) -> bool {
    ensure_restored(model_id).await;
    let mut map = HEALTH.lock().await;
    match map.get_mut(&model_id) {
        Some(sm) if matches!(sm.state, InstanceState::Failed { .. }) => {
            sm.on_event(HealthEvent::ClearFailed);
            true
        }
        _ => false,
    }
}

/// Force-clear the in-flight cell AND the health state machine for a model.
/// Used by the reaper + tests after a drained instance was stopped. Evicting
/// the HEALTH entry here bounds the map to currently-tracked models (otherwise
/// it grows once per model ever auto-started, for the process lifetime). A
/// later restart simply re-creates a fresh state machine.
pub async fn forget(model_id: Uuid) {
    IN_FLIGHT.lock().await.remove(&model_id);
    HEALTH.lock().await.remove(&model_id);
}
