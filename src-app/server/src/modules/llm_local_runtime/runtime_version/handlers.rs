//! API handlers for runtime version management

use aide::axum::IntoApiResponse;
use axum::{extract::{Extension, Path, Query}, http::StatusCode, Json};
use llm_runtime::config::EngineType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::{EventBus, Repos},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::events::LlmLocalRuntimeEvent;
use super::super::permissions::*;
use super::models::*;
use super::super::BinaryManager;

// =====================================================
// Query Parameters
// =====================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListVersionsQuery {
    /// Filter by engine (optional)
    pub engine: Option<String>,
}

// =====================================================
// Handler Functions
// =====================================================

/// List all registered runtime versions
pub async fn list_runtime_versions(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Query(params): Query<ListVersionsQuery>,
) -> ApiResult<Json<RuntimeVersionListResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    let versions = if let Some(engine) = params.engine {
        binary_manager
            .list_versions_for_engine(&engine)
            .await
            .map_err(|e| AppError::internal_error(&format!("Database error: {}", e)))?
    } else {
        binary_manager
            .list_versions()
            .await
            .map_err(|e| AppError::internal_error(&format!("Database error: {}", e)))?
    };

    let response = RuntimeVersionListResponse {
        versions: versions.into_iter().map(RuntimeVersionResponse::from).collect(),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Get a specific runtime version by ID
pub async fn get_runtime_version(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Path(version_id): Path<Uuid>,
) -> ApiResult<Json<RuntimeVersionResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    // Verify binary exists (result is the path, but we only need to check existence)
    let _version = binary_manager
        .get_binary_path(version_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(&format!("Database error: {}", e))
            }
        })?;

    // Get the version record from database
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(&format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;

    Ok((StatusCode::OK, Json(RuntimeVersionResponse::from(version_record))))
}

/// Download and register a runtime version
pub async fn download_runtime_version(
    _auth: RequirePermissions<(RuntimeVersionCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(req): Json<DownloadVersionRequest>,
) -> ApiResult<Json<DownloadVersionResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    // Parse engine type
    let engine = match req.engine.as_str() {
        "llamacpp" => EngineType::Llamacpp,
        "mistralrs" => EngineType::Mistralrs,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request("INVALID_ENGINE", "Engine must be 'llamacpp' or 'mistralrs'"),
            ))
        }
    };

    // Download and register
    let version = binary_manager
        .download_and_register(engine, &req.version, &req.platform, &req.arch, &req.backend)
        .await
        .map_err(|e| {
            tracing::error!("Failed to download runtime version: {}", e);
            AppError::internal_error(&format!("Failed to download runtime version: {}", e))
        })?;

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::runtime_version_downloaded(
            version.id,
            req.engine.clone(),
            req.version.clone(),
        )
        .into(),
    );

    let response = DownloadVersionResponse {
        version: RuntimeVersionResponse::from(version),
        downloaded: true,
        message: format!("Successfully downloaded and registered {} {} for {}/{}/{}",
            req.engine, req.version, req.platform, req.arch, req.backend),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a runtime version
pub async fn delete_runtime_version(
    _auth: RequirePermissions<(RuntimeVersionDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(version_id): Path<Uuid>,
    Query(params): Query<DeleteVersionQuery>,
) -> ApiResult<impl IntoApiResponse> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    let remove_binary = params.remove_binary.unwrap_or(false);

    // Get version info before deletion for event
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(&format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;

    binary_manager
        .delete_version(version_id, remove_binary)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete runtime version: {}", e);
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(&format!("Failed to delete runtime version: {}", e))
            }
        })?;

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::runtime_version_deleted(
            version_id,
            version_record.engine,
            version_record.version,
        )
        .into(),
    );

    Ok((StatusCode::NO_CONTENT, ()))
}

/// Set a runtime version as system default
pub async fn set_system_default(
    _auth: RequirePermissions<(RuntimeVersionUpdate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(version_id): Path<Uuid>,
) -> ApiResult<Json<RuntimeVersionResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    binary_manager
        .set_system_default(version_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to set system default: {}", e);
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(&format!("Failed to set system default: {}", e))
            }
        })?;

    // Get the updated version record
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(&format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;

    // Emit event for cache invalidation
    event_bus.emit_async(
        LlmLocalRuntimeEvent::runtime_version_default_changed(
            version_id,
            version_record.engine.clone(),
            version_record.version.clone(),
        )
        .into(),
    );

    Ok((StatusCode::OK, Json(RuntimeVersionResponse::from(version_record))))
}

/// Check for available updates from GitHub
pub async fn check_for_updates(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Path(engine): Path<String>,
) -> ApiResult<Json<AvailableUpdatesResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    let available_versions = binary_manager
        .check_for_updates(&engine)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check for updates: {}", e);
            AppError::internal_error(&format!("Failed to check for updates: {}", e))
        })?;

    let response = AvailableUpdatesResponse {
        engine,
        available_versions,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Sync cache with database
pub async fn sync_cache(
    _auth: RequirePermissions<(RuntimeVersionUpdate,)>,
) -> ApiResult<Json<SyncCacheResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::new(pool.clone())
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize BinaryManager: {}", e)))?;

    let synced_count = binary_manager
        .sync_cache()
        .await
        .map_err(|e| {
            tracing::error!("Failed to sync cache: {}", e);
            AppError::internal_error(&format!("Failed to sync cache: {}", e))
        })?;

    let response = SyncCacheResponse {
        synced_count,
        message: format!("Synced {} cached binaries to database", synced_count),
    };

    Ok((StatusCode::OK, Json(response)))
}

// =====================================================
// Query Parameter Types
// =====================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteVersionQuery {
    /// Whether to remove the binary file (defaults to false)
    pub remove_binary: Option<bool>,
}

// =====================================================
// OpenAPI Documentation
// =====================================================

pub fn list_runtime_versions_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.list")
        .description("List all registered runtime versions, optionally filtered by engine")
        .tag("Runtime Versions")
        .response::<200, Json<RuntimeVersionListResponse>>()
}

pub fn get_runtime_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.get")
        .description("Get a specific runtime version by ID")
        .tag("Runtime Versions")
        .response::<200, Json<RuntimeVersionResponse>>()
        .response_with::<404, (), _>(|res| {
            res.description("Runtime version not found")
        })
}

pub fn download_runtime_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionCreate,)>(op)
        .id("RuntimeVersion.download")
        .description("Download and register a runtime version from GitHub releases")
        .tag("Runtime Versions")
        .response::<200, Json<DownloadVersionResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description("Invalid request parameters")
        })
}

pub fn delete_runtime_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionDelete,)>(op)
        .id("RuntimeVersion.delete")
        .description("Delete a runtime version from the database and optionally remove the binary file")
        .tag("Runtime Versions")
        .response_with::<204, (), _>(|res| {
            res.description("Runtime version deleted successfully")
        })
        .response_with::<404, (), _>(|res| {
            res.description("Runtime version not found")
        })
}

pub fn set_system_default_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionUpdate,)>(op)
        .id("RuntimeVersion.setDefault")
        .description("Set a runtime version as the system default for its engine")
        .tag("Runtime Versions")
        .response::<200, Json<RuntimeVersionResponse>>()
        .response_with::<404, (), _>(|res| {
            res.description("Runtime version not found")
        })
}

pub fn check_for_updates_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.checkUpdates")
        .description("Check for available runtime version updates from GitHub")
        .tag("Runtime Versions")
        .response::<200, Json<AvailableUpdatesResponse>>()
}

pub fn sync_cache_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionUpdate,)>(op)
        .id("RuntimeVersion.syncCache")
        .description("Sync cached binaries with the database")
        .tag("Runtime Versions")
        .response::<200, Json<SyncCacheResponse>>()
}
