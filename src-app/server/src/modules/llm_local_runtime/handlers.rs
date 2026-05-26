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
    // Check if instance already exists
    let existing = Repos.local_runtime.get_instance_by_model(model_id).await?;
    if let Some(_instance) = existing {
        return Err((
            StatusCode::CONFLICT,
            AppError::conflict("Model instance already running"),
        ));
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

    // Use model name as the path/identifier
    let model_path = model.name.clone();

    // Build engine config. When the model is flagged as an embedder
    // (capabilities.text_embedding=true) we set `embeddings: true` so
    // build_llamacpp_command adds the `--embeddings` flag. Without
    // this, llama-server boots in chat mode and `/embedding` POSTs
    // return 404 / empty vectors. Plan §8 "Local-runtime embedding proxy".
    let mut engine_config = serde_json::json!({});
    if model.capabilities.text_embedding.unwrap_or(false) {
        engine_config["embeddings"] = serde_json::Value::Bool(true);
    }

    // Start the deployment
    let result = deployment
        .start(
            model_id,
            model.engine_type.as_str(),
            &model_path,
            &engine_config,
        )
        .await?;

    // Create database record
    let _instance_id = Repos
        .local_runtime
        .create_instance(
            model_id,
            provider.id,
            result.port,
            &result.base_url,
            None,  // runtime_version_id: will be tracked properly in future iteration
        )
        .await?;

    // Update status to running
    Repos
        .local_runtime
        .update_instance_status(model_id, "running", None)
        .await?;

    // Get and return the created instance
    let instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::internal_error("Failed to retrieve created instance"))?;

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
    // Check that instance exists
    let _instance = Repos
        .local_runtime
        .get_instance_by_model(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;

    // Get deployment strategy and stop (always local)
    let deployment_manager = get_deployment_manager();
    let deployment = deployment_manager.get_deployment(&DeploymentConfig::Local { binary_path: None }).await?;
    deployment.stop(model_id).await?;

    // Update database status
    Repos
        .local_runtime
        .update_instance_status(model_id, "stopped", None)
        .await?;

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

    // Get model details for restart
    let model = Repos
        .llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Use model name as the path/identifier
    let model_path = model.name.clone();

    // Start new deployment
    let result = deployment
        .start(
            model_id,
            model.engine_type.as_str(),
            &model_path,
            &serde_json::json!({}),
        )
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

