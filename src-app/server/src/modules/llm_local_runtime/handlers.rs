// API handlers for local runtime management

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::{EventBus, Repos},
    modules::permissions::RequirePermissions,
};

use super::events::LlmLocalRuntimeEvent;
use super::models::*;
use super::permissions::*;
use super::get_deployment_manager;

// =====================================================
// Model Instance Management Handlers
// =====================================================

pub async fn start_model_instance(
    _auth: RequirePermissions<(LocalRuntimeManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(model_id): Path<Uuid>,
    Json(_req): Json<StartInstanceRequest>,
) -> ApiResult<Json<InstanceResponse>> {
    // Only a genuinely-running, healthy instance blocks a new start. A
    // leftover row in a non-running state — e.g. the `status='stopped'` row
    // that validation's probe leaves behind (validator::teardown_validation_instance),
    // or a prior manual stop, or a crashed engine — must NOT 409 with
    // "already running"; clean it up and start fresh. This mirrors the proxy
    // auto-start path (auto_start::probe_liveness), which only treats a
    // running + health-checked instance as live.
    if let Some(existing) = Repos.local_runtime.get_instance_by_model(model_id).await? {
        if existing.status == "running" {
            let dep = get_deployment_manager()
                .get_deployment(&DeploymentConfig::Local { binary_path: None })
                .await?;
            if dep.health_check(&existing.base_url).await.unwrap_or(false) {
                return Err((
                    StatusCode::CONFLICT,
                    AppError::conflict("Model instance already running"),
                ));
            }
        }
        // Stale / stopped / unhealthy row → drop it so create_instance
        // (model_id is UNIQUE) can re-insert below.
        Repos.local_runtime.delete_instance(model_id).await?;
    }

    // Get model details
    let model = Repos
        .llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Get provider for tracking
    let provider = Repos
        .llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    // Get deployment strategy (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;

    // Resolve the real model file path + typed engine_settings (incl.
    // the embedder `--embeddings` flag derived from capabilities) the
    // same way the proxy auto-start path does — `model.name` is NOT a
    // file path, and the engine knobs live in `engine_settings`.
    let (engine_type, model_path, engine_config) =
        super::auto_start::resolve_model_inputs(Repos.pool(), model_id).await?;

    // Start the deployment
    let result = deployment
        .start(model_id, &engine_type, &model_path, &engine_config)
        .await?;

    // The engine subprocess is now live but untracked. Any failure in the
    // following DB writes (e.g. a UNIQUE(model_id) violation from a concurrent
    // start that won the create_instance race, or a transient DB error) would
    // otherwise LEAK the spawned process with no row to ever stop it. Run the
    // record-creation steps under a compensating-cleanup helper: on the first
    // error, tear the deployment back down before propagating, so we never
    // leave a running engine the system has forgotten about.
    let persist_instance = async {
        // Create database record
        Repos
            .local_runtime
            .create_instance(
                model_id,
                provider.id,
                result.port,
                &result.base_url,
                None, // runtime_version_id: will be tracked properly in future iteration
            )
            .await?;

        // Update status to running
        Repos
            .local_runtime
            .update_instance_status(model_id, "running", None)
            .await?;

        // Get and return the created instance
        Repos
            .local_runtime
            .get_instance_by_model(model_id)
            .await?
            .ok_or_else(|| AppError::internal_error("Failed to retrieve created instance"))
    };

    let instance = match persist_instance.await {
        Ok(instance) => instance,
        Err(e) => {
            // Best-effort teardown of the just-spawned engine so it is not
            // orphaned. Log but do not mask the original error.
            if let Err(stop_err) = deployment.stop(model_id).await {
                tracing::error!(
                    "failed to tear down orphaned engine for model {} after start error: {}",
                    model_id,
                    stop_err
                );
            }
            return Err(e.into());
        }
    };

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::instance_started(
            instance.id,
            instance.model_id,
            instance.provider_id,
        )
        .into(),
    );

    Ok((
        StatusCode::CREATED,
        Json(InstanceResponse {
            id: instance.id,
            model_id: instance.model_id,
            provider_id: instance.provider_id,
            runtime_version_id: instance.runtime_version_id,
            local_port: instance.local_port,
            base_url: instance.base_url,
            status: instance.status,
            error_message: instance.error_message,
            started_at: instance.started_at,
            last_health_check: instance.last_health_check,
            stopped_at: instance.stopped_at,
        }),
    ))
}

pub fn start_model_instance_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeManage,)>(op)
        .id("LocalRuntime.startModel")
        .description("Start a local runtime instance for a model")
        .tag("LocalRuntime")
        .response::<201, Json<InstanceResponse>>()
}

pub async fn stop_model_instance(
    _auth: RequirePermissions<(LocalRuntimeManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<InstanceResponse>> {
    // Update the DB status FIRST so that if this fails (e.g. the instance
    // does not exist), we never attempt the external side-effect of killing
    // the process.  This avoids the "multi-step write" hazard where the
    // process is dead but the DB still says "running".
    Repos
        .local_runtime
        .update_instance_status(model_id, "stopped", None)
        .await?;

    // Get deployment strategy and stop (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;
    deployment.stop(model_id).await?;

    // Get and return the updated instance
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::internal_error("Failed to retrieve instance"))?;

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::instance_stopped(instance.id, instance.model_id).into(),
    );

    Ok((
        StatusCode::OK,
        Json(InstanceResponse {
            id: instance.id,
            model_id: instance.model_id,
            provider_id: instance.provider_id,
            runtime_version_id: instance.runtime_version_id,
            local_port: instance.local_port,
            base_url: instance.base_url,
            status: instance.status,
            error_message: instance.error_message,
            started_at: instance.started_at,
            last_health_check: instance.last_health_check,
            stopped_at: instance.stopped_at,
        }),
    ))
}

pub fn stop_model_instance_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeManage,)>(op)
        .id("LocalRuntime.stopModel")
        .description("Stop a running instance")
        .tag("LocalRuntime")
        .response::<200, Json<InstanceResponse>>()
}

pub async fn restart_model_instance(
    _auth: RequirePermissions<(LocalRuntimeManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<InstanceResponse>> {
    // Get the existing instance
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;

    // Get provider for tracking
    let provider_id = instance.provider_id;

    // Stop the deployment (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;
    deployment.stop(model_id).await?;

    // Delete the instance record
    Repos.local_runtime.delete_instance(model_id).await?;

    // Resolve the real model file path + typed engine_settings (same
    // source as the auto-start path); 404s if the model/files are gone.
    let (engine_type, model_path, engine_config) =
        super::auto_start::resolve_model_inputs(Repos.pool(), model_id).await?;

    // Start new deployment
    let result = deployment
        .start(model_id, &engine_type, &model_path, &engine_config)
        .await?;

    // Create new database record
    Repos
        .local_runtime
        .create_instance(
            model_id,
            provider_id,
            result.port,
            &result.base_url,
            None,  // runtime_version_id: will be tracked properly in future iteration
        )
        .await?;

    // Update status
    Repos
        .local_runtime
        .update_instance_status(model_id, "running", None)
        .await?;

    // Get and return the new instance
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::internal_error("Failed to retrieve restarted instance"))?;

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::instance_restarted(instance.id, instance.model_id).into(),
    );

    Ok((
        StatusCode::OK,
        Json(InstanceResponse {
            id: instance.id,
            model_id: instance.model_id,
            provider_id: instance.provider_id,
            runtime_version_id: instance.runtime_version_id,
            local_port: instance.local_port,
            base_url: instance.base_url,
            status: instance.status,
            error_message: instance.error_message,
            started_at: instance.started_at,
            last_health_check: instance.last_health_check,
            stopped_at: instance.stopped_at,
        }),
    ))
}

pub fn restart_model_instance_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeManage,)>(op)
        .id("LocalRuntime.restartModel")
        .description("Restart an instance")
        .tag("LocalRuntime")
        .response::<200, Json<InstanceResponse>>()
}

/// Swap a model onto another version **of the same engine** (the engine type
/// itself cannot change). Pins the model's `required_runtime_version_id`; if
/// the model is currently running, restarts it on the new version so the
/// change takes effect immediately.
pub async fn swap_model_runtime_version(
    _auth: RequirePermissions<(LocalRuntimeManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(model_id): Path<Uuid>,
    Json(req): Json<super::runtime_version::models::SwapRuntimeVersionRequest>,
) -> ApiResult<Json<super::runtime_version::models::SwapRuntimeVersionResponse>> {
    let pool = Repos.pool();

    // The model's engine (as stored text) — read directly so we don't depend
    // on the model entity's enum representation.
    let model_engine: String = sqlx::query_scalar!(
        "SELECT engine_type FROM llm_models WHERE id = $1",
        model_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("model lookup: {e}")))?
    .ok_or_else(|| AppError::not_found("Model"))?;

    // Target version must be the SAME engine — swap version, not engine.
    let target = super::runtime_version::repository::get_by_id(pool, req.version_id)
        .await
        .map_err(|e| AppError::internal_error(format!("version lookup: {e}")))?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;
    if target.engine != model_engine {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "ENGINE_MISMATCH",
                format!(
                    "Cannot swap a '{model_engine}' model onto a '{}' version; \
                     only the engine version may change, not the engine.",
                    target.engine
                ),
            ),
        ));
    }

    // Pin the model to the target version.
    super::runtime_version::repository::set_model_runtime_version(
        pool,
        model_id,
        Some(req.version_id),
    )
    .await
    .map_err(|e| AppError::internal_error(format!("set version: {e}")))?;

    // If a running instance exists, restart it onto the new version (mirrors
    // restart_model_instance: stop → drop record → re-resolve → start).
    let mut restarted = false;
    if let Some(instance) = Repos.local_runtime.get_instance_by_model(model_id).await? {
        let provider_id = instance.provider_id;
        let deployment_manager = get_deployment_manager();
        let deployment = deployment_manager
            .get_deployment(&DeploymentConfig::Local { binary_path: None })
            .await?;
        deployment.stop(model_id).await?;
        Repos.local_runtime.delete_instance(model_id).await?;

        let (engine_type, model_path, engine_config) =
            super::auto_start::resolve_model_inputs(pool, model_id).await?;
        let result = deployment
            .start(model_id, &engine_type, &model_path, &engine_config)
            .await?;
        Repos
            .local_runtime
            .create_instance(model_id, provider_id, result.port, &result.base_url, None)
            .await?;
        Repos
            .local_runtime
            .update_instance_status(model_id, "running", None)
            .await?;
        restarted = true;

        event_bus.emit_async(
            LlmLocalRuntimeEvent::instance_restarted(instance.id, model_id).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(super::runtime_version::models::SwapRuntimeVersionResponse {
            model_id,
            version_id: req.version_id,
            restarted,
        }),
    ))
}

pub fn swap_model_runtime_version_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeManage,)>(op)
        .id("LocalRuntime.swapModelVersion")
        .description("Swap a model onto another version of the same engine (restarts if running)")
        .tag("LocalRuntime")
        .response::<200, Json<super::runtime_version::models::SwapRuntimeVersionResponse>>()
}

pub async fn get_model_instance(
    _auth: RequirePermissions<(LocalRuntimeRead,)>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<InstanceResponse>> {
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;

    Ok((
        StatusCode::OK,
        Json(InstanceResponse {
            id: instance.id,
            model_id: instance.model_id,
            provider_id: instance.provider_id,
            runtime_version_id: instance.runtime_version_id,
            local_port: instance.local_port,
            base_url: instance.base_url,
            status: instance.status,
            error_message: instance.error_message,
            started_at: instance.started_at,
            last_health_check: instance.last_health_check,
            stopped_at: instance.stopped_at,
        }),
    ))
}

pub fn get_model_instance_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeRead,)>(op)
        .id("LocalRuntime.getInstance")
        .description("Get instance details by model ID")
        .tag("LocalRuntime")
        .response::<200, Json<InstanceResponse>>()
}

pub async fn get_model_status(
    auth: RequirePermissions<(LocalRuntimeRead,)>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<InstanceStatusResponse>> {
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?;

    if let Some(inst) = instance {
        // Get deployment strategy and check status (always local)
        let deployment_manager = get_deployment_manager();
        let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;
        let status = deployment.status(model_id).await?;

        // Redact base_url (which includes the local port) for
        // non-admin callers. Closes 08-llm-local-runtime F-13 (Low).
        // With F-04's per-instance bearer token in place, knowing the
        // port is no longer sufficient to reach the engine, but the
        // info-disclosure is still avoidable.
        let base_url = if auth.user.is_admin {
            Some(inst.base_url)
        } else {
            None
        };

        Ok((
            StatusCode::OK,
            Json(InstanceStatusResponse {
                model_id,
                status: if status.running { "running" } else { "stopped" }.to_string(),
                base_url,
                uptime_seconds: status.uptime_seconds,
            }),
        ))
    } else {
        Ok((
            StatusCode::OK,
            Json(InstanceStatusResponse {
                model_id,
                status: "not_found".to_string(),
                base_url: None,
                uptime_seconds: None,
            }),
        ))
    }
}

pub fn get_model_status_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeRead,)>(op)
        .id("LocalRuntime.getStatus")
        .description("Get instance status")
        .tag("LocalRuntime")
        .response::<200, Json<InstanceStatusResponse>>()
}

pub async fn get_model_health(
    _auth: RequirePermissions<(LocalRuntimeRead,)>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<HealthCheckResponse>> {
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;

    // Get deployment strategy (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;

    let start_time = std::time::Instant::now();
    let healthy = deployment.health_check(&instance.base_url).await?;
    let response_time_ms = start_time.elapsed().as_millis() as u64;

    Ok((
        StatusCode::OK,
        Json(HealthCheckResponse {
            healthy,
            message: if healthy {
                Some("Instance is healthy".to_string())
            } else {
                Some("Instance is unhealthy".to_string())
            },
            response_time_ms: Some(response_time_ms),
        }),
    ))
}

pub fn get_model_health_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeRead,)>(op)
        .id("LocalRuntime.healthCheck")
        .description("Health check for instance")
        .tag("LocalRuntime")
        .response::<200, Json<HealthCheckResponse>>()
}

pub async fn get_model_logs(
    _auth: RequirePermissions<(LocalRuntimeLogs,)>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<Json<LogsResponse>> {
    // Check that instance exists
    let _instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;

    // Get deployment strategy (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;

    let logs = deployment.get_logs(model_id, 100).await?;

    Ok((
        StatusCode::OK,
        Json(LogsResponse { model_id, logs }),
    ))
}

pub fn get_model_logs_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeLogs,)>(op)
        .id("LocalRuntime.getLogs")
        .description("Get instance logs")
        .tag("LocalRuntime")
        .response::<200, Json<LogsResponse>>()
}

// =====================================================
// P3: GPU detection
// =====================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GpuDetectionResponse {
    /// All usable backends on this host. Always includes "cpu".
    pub available: Vec<String>,
    /// Recommended backend (priority winner).
    pub recommended: String,
    pub platform: String,
    pub arch: String,
}

/// GET /api/local-runtime/detect-gpu
///
/// Powers the settings-page GPU detection card: surfaces which
/// engine backend(s) the host supports + the recommended pick so
/// the runtime download drawer can pre-fill backend + arch.
pub async fn detect_gpu(
    _auth: RequirePermissions<(LocalRuntimeRead,)>,
) -> ApiResult<Json<GpuDetectionResponse>> {
    let d = super::utils::gpu_detect::detect_all();
    Ok((
        StatusCode::OK,
        Json(GpuDetectionResponse {
            available: d.available,
            recommended: d.recommended,
            platform: d.platform,
            arch: d.arch,
        }),
    ))
}

pub fn detect_gpu_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeRead,)>(op)
        .id("LocalRuntime.detectGpu")
        .description("Detect available GPU backends + recommended pick for engine downloads")
        .tag("LocalRuntime")
        .response::<200, Json<GpuDetectionResponse>>()
}

/// P2: SSE log streaming. Subscribes to the per-instance log
/// broadcaster + replays the existing snapshot on connect. Events are
/// the typed `SSELogEvent` enum (`log` / `lag`) so the generated TS
/// client gets a fully-typed `SSECallback` — no `as never` casts.
pub async fn stream_model_logs(
    _auth: RequirePermissions<(LocalRuntimeLogs,)>,
    Path(model_id): Path<Uuid>,
) -> ApiResult<
    axum::response::Sse<
        impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>,
    >,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let deployment_manager = get_deployment_manager();
    let dep = deployment_manager
        .get_deployment(&DeploymentConfig::Local { binary_path: None })
        .await?;
    let (mut rx, snapshot) = dep.subscribe_logs(model_id).await?;

    let stream = async_stream::stream! {
        // Replay the buffered snapshot first.
        for line in snapshot {
            let ev: Event = SSELogEvent::Log(SSELogLineData { line }).into();
            yield Ok(ev);
        }
        // Then stream live lines from the broadcaster.
        loop {
            match rx.recv().await {
                Ok(line) => {
                    let ev: Event = SSELogEvent::Log(SSELogLineData { line }).into();
                    yield Ok(ev);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let ev: Event = SSELogEvent::Lag(SSELogLagData {
                        message: format!("dropped {skipped} log line(s)"),
                        dropped: skipped,
                    })
                    .into();
                    yield Ok(ev);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok((
        StatusCode::OK,
        Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(30))),
    ))
}

pub fn stream_model_logs_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeLogs,)>(op)
        .id("LocalRuntime.streamLogs")
        .description("Stream instance logs as Server-Sent Events")
        .tag("LocalRuntime")
        .response::<200, Json<SSELogEvent>>()
}

// =====================================================
// Provider Instances Handler
// =====================================================

pub async fn get_provider_instances(
    _auth: RequirePermissions<(LocalRuntimeRead,)>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<ProviderInstancesResponse>> {
    let instances = Repos
        .local_runtime
        .get_instances_by_provider(provider_id)
        .await?;

    let instance_responses: Vec<InstanceResponse> = instances
        .into_iter()
        .map(|inst| InstanceResponse {
            id: inst.id,
            model_id: inst.model_id,
            provider_id: inst.provider_id,
            runtime_version_id: inst.runtime_version_id,
            local_port: inst.local_port,
            base_url: inst.base_url,
            status: inst.status,
            error_message: inst.error_message,
            started_at: inst.started_at,
            last_health_check: inst.last_health_check,
            stopped_at: inst.stopped_at,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(ProviderInstancesResponse {
            provider_id,
            instances: instance_responses,
        }),
    ))
}

pub fn get_provider_instances_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    use crate::modules::permissions::with_permission;
    with_permission::<(LocalRuntimeRead,)>(op)
        .id("LocalRuntime.getProviderInstances")
        .description("Get all instances for a provider")
        .tag("LocalRuntime")
        .response::<200, Json<ProviderInstancesResponse>>()
}

