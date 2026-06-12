//! Background validation queue for local model files.
//!
//! Tier-2 default (engine load probe): spawn the engine with the
//! model, wait for `/health` = Ok within 90s, then SIGTERM. Catches
//! corruption past the GGUF header, version mismatches, insufficient
//! VRAM, broken chat templates that prevent load.
//!
//! Tier-3 opt-in: same as Tier-2 plus a tiny chat round-trip.
//!
//! Validation is **serialized** across the server — loading a model
//! into RAM/VRAM is heavyweight and concurrent OOM is real. The
//! queue is a single tokio task that picks work off a Mutex<VecDeque>.

use std::collections::VecDeque;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::sync::Mutex;

use crate::common::AppError;
use crate::modules::llm_model::permissions::LlmModelsRead;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

const TIER2_HEALTH_DEADLINE_SECS: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationTier {
    Tier2,
    Tier3,
}

#[derive(Debug, Clone)]
pub enum ValidationOutcome {
    Valid,
    Warning(serde_json::Value),
}

/// Serialized validation queue. One queue per server process.
static QUEUE: LazyLock<Arc<Mutex<VecDeque<(Uuid, ValidationTier)>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(VecDeque::new())));

/// Spawn the background worker. Idempotent — start once at module
/// init and let it run for the server's lifetime.
pub fn spawn_worker(pool: Arc<PgPool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("llm_local_runtime::validator: worker started");
        loop {
            let next = {
                let mut q = QUEUE.lock().await;
                q.pop_front()
            };

            match next {
                Some((model_id, tier)) => {
                    // Run each validation in its own task so a panic
                    // (e.g. deep in a model-file parse) is isolated and
                    // doesn't kill the worker loop — otherwise one bad
                    // model would stop ALL future validations for the
                    // process lifetime (M3).
                    let pool_clone = pool.clone();
                    let join = tokio::spawn(async move {
                        run_validation(&pool_clone, model_id, tier).await
                    })
                    .await;
                    match join {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => tracing::warn!(
                            "validator: model {model_id} tier {tier:?} failed: {e}"
                        ),
                        Err(join_err) => tracing::error!(
                            "validator: model {model_id} tier {tier:?} PANICKED: {join_err}"
                        ),
                    }
                }
                None => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    })
}

/// Enqueue a validation request. The background worker picks it up.
///
/// Honors a **debug-only** opt-out env var
/// `ZIEE_DISABLE_MODEL_VALIDATION=1`. When set, the call short-
/// circuits to a no-op (and writes `valid` directly to the model
/// row so the UI doesn't show "Validating…" forever).
///
/// Use case: E2E tests that drive the engine themselves cannot race
/// against the validator's 90s spawn/kill cycle (the engine spawned
/// for validation conflicts with the test's Start click → 409
/// "already running" + UI sees an unstable Stop button). The env
/// read is compiled out of release builds via `cfg!(debug_assertions)`
/// so production behavior is unchanged.
pub async fn enqueue(model_id: Uuid, tier: ValidationTier) {
    #[cfg(debug_assertions)]
    if std::env::var("ZIEE_DISABLE_MODEL_VALIDATION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        tracing::info!(
            "validator: ZIEE_DISABLE_MODEL_VALIDATION=1 — skipping {tier:?} for model {model_id}"
        );
        return;
    }
    let mut q = QUEUE.lock().await;
    q.push_back((model_id, tier));
    tracing::info!("validator: enqueued model {model_id} tier {tier:?}");
}

/// Validate a REMOTE provider model with a tiny chat round-trip. Unlike
/// local validation (which loads an engine), this just sends a small
/// `messages: [{role:user, content:"Hi"}]` request through the
/// provider's configured client. Catches wrong API key (401), wrong
/// base_url (connection refused), unknown model (404), provider
/// downtime (5xx). Runs inline (not queued) — there's no engine to
/// serialize against. Writes validation_status + validation_issues.
pub async fn validate_remote_model(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<ValidationOutcome, AppError> {
    use ai_providers::{ChatMessage, ChatRequest, Provider};
    use futures::StreamExt;

    let started = std::time::Instant::now();

    // Resolve model + provider.
    let row = sqlx::query!(
        "SELECT name, provider_id FROM llm_models WHERE id = $1",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("validator: model: {e}")))?;
    let row = row.ok_or_else(|| AppError::not_found("model not found"))?;
    let (model_name, provider_id) = (row.name, row.provider_id);

    let provider = crate::core::Repos
        .llm_provider
        .get_by_id(provider_id)
        .await
        .map_err(|e| AppError::internal_error(format!("validator: provider: {e}")))?
        .ok_or_else(|| AppError::not_found("provider not found"))?;

    let api_key = provider.api_key.clone().unwrap_or_default();
    let base_url = provider.base_url.clone().unwrap_or_default();

    let outcome = async {
        let p = Provider::new(&provider.provider_type, &api_key, &base_url)
            .map_err(|e| format!("provider init: {e}"))?;
        let req = ChatRequest {
            model: model_name.clone(),
            messages: vec![ChatMessage::user("Hi")],
            max_tokens: Some(5),
            ..Default::default()
        };
        let mut stream = p.chat_stream(req).await.map_err(|e| format!("{e}"))?;
        // Pull at least one chunk to confirm the round-trip works.
        match stream.next().await {
            Some(Ok(_)) => Ok(()),
            Some(Err(e)) => Err(format!("stream error: {e}")),
            None => Err("empty response stream".to_string()),
        }
    }
    .await;

    let result = match outcome {
        Ok(()) => ValidationOutcome::Valid,
        Err(reason) => ValidationOutcome::Warning(serde_json::json!({
            "phase": "remote_test_inference",
            "reason": reason,
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "detected_at": chrono::Utc::now().to_rfc3339(),
        })),
    };

    let (status, issues) = match &result {
        ValidationOutcome::Valid => ("valid", serde_json::Value::Null),
        ValidationOutcome::Warning(p) => ("validation_warning", p.clone()),
    };
    let issues_json: Option<serde_json::Value> = if issues.is_null() {
        None
    } else {
        Some(serde_json::Value::Array(vec![issues]))
    };
    let _ = sqlx::query!(
        "UPDATE llm_models SET validation_status = $1, validation_issues = $2, updated_at = NOW()
         WHERE id = $3",
        status,
        issues_json,
        model_id,
    )
    .execute(pool)
    .await;

    // Detached validation path: notify admin devices that the model's
    // validation_status changed (LlmModel is permission-scoped → owner None).
    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model_id, Audience::perm::<LlmModelsRead>(), None);

    Ok(result)
}

/// Run a single validation pass.
async fn run_validation(
    pool: &PgPool,
    model_id: Uuid,
    tier: ValidationTier,
) -> Result<ValidationOutcome, AppError> {
    let started_at = std::time::Instant::now();

    // Mark processing
    let _ = sqlx::query!(
        "UPDATE llm_models SET validation_status = 'processing', updated_at = NOW() WHERE id = $1",
        model_id,
    )
    .execute(pool)
    .await;
    // Surface the "Validating…" transition to other admin devices (this runs
    // seconds-to-90s after the trigger handler already returned 202).
    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model_id, Audience::perm::<LlmModelsRead>(), None);

    let outcome = run_tier_internal(pool, model_id, tier, started_at).await;
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    let (status, issues) = match &outcome {
        Ok(ValidationOutcome::Valid) => ("valid", serde_json::Value::Null),
        Ok(ValidationOutcome::Warning(payload)) => ("validation_warning", payload.clone()),
        Err(e) => (
            "validation_warning",
            serde_json::json!({
                "phase": "internal",
                "reason": format!("{e}"),
                "elapsed_ms": elapsed_ms,
                "detected_at": chrono::Utc::now().to_rfc3339(),
            }),
        ),
    };

    let issues_json: Option<serde_json::Value> = if issues.is_null() {
        None
    } else {
        Some(serde_json::Value::Array(vec![issues.clone()]))
    };
    let _ = sqlx::query!(
        "UPDATE llm_models SET validation_status = $1,
                                validation_issues = $2,
                                updated_at = NOW()
         WHERE id = $3",
        status,
        issues_json,
        model_id,
    )
    .execute(pool)
    .await;

    // Also extract + persist capabilities for local models.
    if matches!(tier, ValidationTier::Tier2 | ValidationTier::Tier3) {
        let _ = extract_and_persist_capabilities(pool, model_id).await;
    }

    // Terminal transition (validation_status + capabilities now written) —
    // notify admin devices to refresh the model row.
    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model_id, Audience::perm::<LlmModelsRead>(), None);

    outcome
}

/// Tier-2 body: spawn engine via auto-start machinery, wait for
/// Healthy, immediately stop. Reuses auto_start::ensure_running so
/// the singleflight + state-machine + bind probe all apply.
async fn run_tier_internal(
    pool: &PgPool,
    model_id: Uuid,
    tier: ValidationTier,
    started_at: std::time::Instant,
) -> Result<ValidationOutcome, AppError> {
    let start_res =
        tokio::time::timeout(Duration::from_secs(TIER2_HEALTH_DEADLINE_SECS), async {
            super::auto_start::ensure_running(pool, model_id).await
        })
        .await;

    match start_res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            // ensure_running already stops the engine on its own
            // failure/timeout path AND never persists a running row
            // unless it reached Healthy, so no teardown needed here.
            return Ok(ValidationOutcome::Warning(serde_json::json!({
                "phase": "engine_load",
                "reason": format!("{e}"),
                "elapsed_ms": started_at.elapsed().as_millis() as u64,
                "detected_at": chrono::Utc::now().to_rfc3339(),
            })));
        }
        Err(_) => {
            // Outer timeout fired while ensure_running was still
            // working — best-effort teardown in case it had already
            // persisted a running row before we gave up.
            teardown_validation_instance(pool, model_id).await;
            return Ok(ValidationOutcome::Warning(serde_json::json!({
                "phase": "engine_load",
                "reason": format!(
                    "engine did not reach Healthy in {}s",
                    TIER2_HEALTH_DEADLINE_SECS
                ),
                "elapsed_ms": started_at.elapsed().as_millis() as u64,
                "detected_at": chrono::Utc::now().to_rfc3339(),
            })));
        }
    }

    // Engine is now running with a persisted `status='running'` row
    // (auto_start::persist_instance). EVERY exit path below must run
    // teardown so we don't leave a stale running row pointing at the
    // about-to-be-killed port (B2: a later chat would see
    // already_running=true and forward to a dead port → 502).

    // Tier 3: send a tiny chat round-trip.
    if tier == ValidationTier::Tier3 {
        if let Err(e) = tiny_chat_probe(pool, model_id).await {
            teardown_validation_instance(pool, model_id).await;
            return Ok(ValidationOutcome::Warning(serde_json::json!({
                "phase": "test_inference",
                "reason": format!("{e}"),
                "elapsed_ms": started_at.elapsed().as_millis() as u64,
                "detected_at": chrono::Utc::now().to_rfc3339(),
            })));
        }
    }

    teardown_validation_instance(pool, model_id).await;
    Ok(ValidationOutcome::Valid)
}

/// Stop the validation engine AND reconcile all the state
/// `auto_start::persist_instance` created, mirroring the reaper's
/// cleanup (reaper.rs). Without the DB row update, the instance row
/// stays `status='running'` pointing at a dead port → the proxy's
/// `already_running` fast path forwards a later chat to nothing.
async fn teardown_validation_instance(pool: &PgPool, model_id: Uuid) {
    if let Ok(dep) = super::get_deployment_manager()
        .get_deployment(
            &crate::modules::llm_local_runtime::models::DeploymentConfig::Local {
                binary_path: None,
            },
        )
        .await
    {
        let _ = dep.stop(model_id).await;
    }

    // Reconcile the DB row: validation isn't a persistent run.
    let _ = sqlx::query!(
        "UPDATE llm_runtime_instances
         SET status = 'stopped', state = 'stopped',
             state_changed_at = NOW(), stopped_at = NOW()
         WHERE model_id = $1",
        model_id,
    )
    .execute(pool)
    .await;

    // Clear in-memory proxy tracking so a future auto-start is clean.
    // (Not forget_inflight — see H1/H2; the counter persists.)
    super::proxy::clear_instance_flag(model_id).await;
    super::auto_start::forget(model_id).await;
}

async fn tiny_chat_probe(pool: &PgPool, model_id: Uuid) -> Result<(), AppError> {
    // Resolve the running instance's port + bearer.
    let port: Option<i32> = sqlx::query_scalar!(
        "SELECT local_port FROM llm_runtime_instances
         WHERE model_id = $1 AND status = 'running'",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("validator: instance port: {e}")))?;
    let port = port.ok_or_else(|| AppError::internal_error("no running port"))?;

    let bearer =
        super::deployment::local::get_instance_api_key(model_id).unwrap_or_default();

    let model_name: String = sqlx::query_scalar!(
        "SELECT name FROM llm_models WHERE id = $1",
        model_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("validator: model name: {e}")))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::internal_error(format!("reqwest: {e}")))?;
    let body = serde_json::json!({
        "model": model_name,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 5,
        "stream": false,
    });
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
        .bearer_auth(&bearer)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("tiny chat POST: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "tiny chat returned {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Run metadata extraction on the model's file_path and write the
/// resulting capabilities JSONB. Logs (does not fail validation) on
/// extraction error — partial capability info is better than none.
async fn extract_and_persist_capabilities(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<(), AppError> {
    // engine_type lives on llm_models; the file path lives in
    // llm_model_files (a model may have several files).
    let model = sqlx::query!(
        "SELECT engine_type FROM llm_models WHERE id = $1",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("validator: cap model query: {e}")))?;
    let Some(model) = model else {
        return Ok(());
    };
    let engine_type = model.engine_type;

    let files = sqlx::query!(
        "SELECT file_path FROM llm_model_files WHERE model_id = $1 ORDER BY uploaded_at",
        model_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("validator: cap files query: {e}")))?;
    if files.is_empty() {
        return Ok(());
    }
    // Same path-resolution as auto_start: prefer a .gguf file, else
    // the containing directory.
    let file_path = match files.iter().find(|f| f.file_path.ends_with(".gguf")) {
        Some(f) => f.file_path.clone(),
        None => std::path::Path::new(&files[0].file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| files[0].file_path.clone()),
    };

    use crate::modules::llm_local_runtime::engine;
    let engine_ty = match engine_type.as_str() {
        "llamacpp" => engine::EngineType::Llamacpp,
        "mistralrs" => engine::EngineType::Mistralrs,
        _ => return Ok(()),
    };

    let path = std::path::PathBuf::from(&file_path);
    let caps = match engine::extract_model_capabilities(&path, engine_ty) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("validator: capability extraction failed for {model_id}: {e}");
            engine::ModelCapabilities {
                auto_detection_failed: Some(true),
                error: Some(format!("{e}")),
                ..Default::default()
            }
        }
    };

    let caps_json = serde_json::to_value(caps).unwrap_or(serde_json::Value::Null);
    let _ = sqlx::query!(
        "UPDATE llm_models SET capabilities = $1, updated_at = NOW() WHERE id = $2",
        caps_json,
        model_id,
    )
    .execute(pool)
    .await;
    Ok(())
}
