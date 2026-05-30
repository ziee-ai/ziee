//! API handlers for runtime version management

use aide::axum::IntoApiResponse;
use axum::{extract::{Extension, Path, Query}, http::StatusCode, Json};
use crate::modules::llm_local_runtime::engine::EngineType;
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

/// A backend artifact tag: starts with a lowercase letter, then `[a-z0-9.]`
/// (e.g. `cpu`, `cuda12.6`, `rocm6.1`). Rejects `..` / path separators.
fn is_valid_backend(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
        && !s.contains("..")
}

/// A release tag: starts alphanumeric, then `[A-Za-z0-9._-]` (e.g.
/// `v0.0.1-alpha`, `latest`, `b4359`). Rejects `..`, `/`, leading `-`.
fn is_valid_release_tag(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !s.contains("..")
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    let versions = if let Some(engine) = params.engine {
        binary_manager
            .list_versions_for_engine(&engine)
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
    } else {
        binary_manager
            .list_versions()
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    // Verify binary exists (result is the path, but we only need to check existence)
    let _version = binary_manager
        .get_binary_path(version_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(format!("Database error: {}", e))
            }
        })?;

    // Get the version record from database
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

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

    // Validate the remaining free-form fields BEFORE they reach the cache
    // path + download URL. Unchecked, a `../` in version/platform would
    // traverse out of the binaries cache dir (and the release URL path) and
    // the resolved binary_path is later spawned as a child process — so this
    // is a hard input-validation boundary, not cosmetic.
    if !matches!(req.platform.as_str(), "linux" | "macos" | "windows") {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_PLATFORM", "Platform must be linux, macos, or windows"),
        ));
    }
    if !matches!(req.arch.as_str(), "x86_64" | "aarch64") {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_ARCH", "Architecture must be x86_64 or aarch64"),
        ));
    }
    if !is_valid_backend(&req.backend) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_BACKEND",
                "Backend must start with a lowercase letter and contain only [a-z0-9.] (e.g. cpu, cuda12.6)",
            ),
        ));
    }
    if !is_valid_release_tag(&req.version) {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_VERSION",
                "Version must be a release tag of [A-Za-z0-9._-] (e.g. v0.0.1-alpha, latest) with no path separators",
            ),
        ));
    }

    // Download and register
    let version = binary_manager
        .download_and_register(engine, &req.version, &req.platform, &req.arch, &req.backend)
        .await
        .map_err(|e| {
            tracing::error!("Failed to download runtime version: {}", e);
            AppError::internal_error(format!("Failed to download runtime version: {}", e))
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    let remove_binary = params.remove_binary.unwrap_or(false);

    // Get version info before deletion for event
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;

    // In-use guard. The runtime_version FKs are ON DELETE SET NULL, so the DB
    // would silently orphan dependents (breaking auto-start) rather than
    // erroring. Refuse with 409 when any model EFFECTIVELY resolves to this
    // version (same chain as the usage view: model pin → provider default →
    // system default → latest), or a provider defaults to it, or a running
    // instance backs it. A version with no effective dependents is deletable
    // even if it is the current system default (nothing breaks — unpinned
    // models just re-resolve to the next default/latest).
    let local_models =
        crate::modules::llm_local_runtime::runtime_version::repository::list_local_models_with_status(
            pool,
            Some(&version_record.engine),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("usage models query: {e}")),
            )
        })?;
    let mut effective_models = 0u32;
    for m in &local_models {
        let resolved = binary_manager
            .select_runtime_version(Some(m.id), Some(m.provider_id), &version_record.engine)
            .await
            .ok()
            .flatten();
        if resolved.map(|v| v.id) == Some(version_id) {
            effective_models += 1;
        }
    }
    let usage =
        crate::modules::llm_local_runtime::runtime_version::repository::usage(pool, version_id)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error(format!("usage check: {e}")),
                )
            })?;
    if effective_models > 0 || usage.providers > 0 || usage.running_instances > 0 {
        return Err((
            StatusCode::CONFLICT,
            AppError::new(
                StatusCode::CONFLICT,
                "VERSION_IN_USE",
                format!(
                    "Cannot delete: this runtime version is used by {} model(s), \
                     {} provider default(s), and {} running instance(s). Swap them \
                     to another version (or set a different default) first.",
                    effective_models, usage.providers, usage.running_instances
                ),
            ),
        ));
    }

    binary_manager
        .delete_version(version_id, remove_binary)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete runtime version: {}", e);
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(format!("Failed to delete runtime version: {}", e))
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    binary_manager
        .set_system_default(version_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to set system default: {}", e);
            if e.to_string().contains("not found") {
                AppError::not_found("Runtime version")
            } else {
                AppError::internal_error(format!("Failed to set system default: {}", e))
            }
        })?;

    // Get the updated version record
    let version_record = crate::modules::llm_local_runtime::runtime_version::repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?
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
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    // Asset-readiness is host-specific: a release may ship the cpu binary
    // for one platform/arch before another. Compute the diff for THIS host,
    // detected at runtime (not the compile-time target).
    let platform = super::super::utils::gpu_detect::host_platform();
    let arch = super::super::utils::gpu_detect::host_arch();

    let versions = binary_manager
        .check_for_updates(&engine, &platform, &arch)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check for updates: {}", e);
            AppError::internal_error(format!("Failed to check for updates: {}", e))
        })?;

    let response = AvailableUpdatesResponse {
        engine,
        platform,
        arch,
        versions,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// List local models grouped by the engine version they effectively use.
///
/// "Effective" follows the same fallback chain the runtime uses to start a
/// model (model pin → provider default → system default → latest), so the
/// view reflects what each model actually runs on, not just explicit pins.
pub async fn list_version_usage(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Query(params): Query<ListVersionsQuery>,
) -> ApiResult<Json<VersionUsageResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::with_cache_dir(
        pool.clone(),
        std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()),
    )
    .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    let engine = params.engine.as_deref();
    let models = super::repository::list_local_models_with_status(pool, engine)
        .await
        .map_err(|e| AppError::internal_error(format!("usage query: {e}")))?;

    let versions = match engine {
        Some(e) => super::repository::list_by_engine(pool, e).await,
        None => super::repository::list_all(pool).await,
    }
    .map_err(|e| AppError::internal_error(format!("versions query: {e}")))?;

    let mut by_version: std::collections::HashMap<Uuid, Vec<ModelUsageInfo>> =
        std::collections::HashMap::new();
    let mut unresolved: Vec<ModelUsageInfo> = Vec::new();

    for m in models {
        let effective = binary_manager
            .select_runtime_version(Some(m.id), Some(m.provider_id), &m.engine)
            .await
            .ok()
            .flatten();
        let pinned = match &effective {
            Some(v) => m.required_runtime_version_id == Some(v.id),
            None => false,
        };
        let info = ModelUsageInfo {
            id: m.id,
            name: m.name,
            display_name: m.display_name,
            provider_id: m.provider_id,
            provider_name: m.provider_name,
            engine: m.engine,
            running: m.running,
            pinned,
        };
        match effective {
            Some(v) => by_version.entry(v.id).or_default().push(info),
            None => unresolved.push(info),
        }
    }

    let entries = versions
        .into_iter()
        .map(|v| {
            let models = by_version.remove(&v.id).unwrap_or_default();
            VersionUsageEntry {
                version: RuntimeVersionResponse::from(v),
                models,
            }
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(VersionUsageResponse {
            versions: entries,
            unresolved,
        }),
    ))
}

pub fn list_version_usage_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.usage")
        .description("List local models grouped by the engine version they effectively use")
        .tag("LocalRuntime")
        .response::<200, Json<VersionUsageResponse>>()
}

/// Sync cache with database
pub async fn sync_cache(
    _auth: RequirePermissions<(RuntimeVersionUpdate,)>,
) -> ApiResult<Json<SyncCacheResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::with_cache_dir(pool.clone(), std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()))
        .map_err(|e| AppError::internal_error(format!("Failed to initialize BinaryManager: {}", e)))?;

    let synced_count = binary_manager
        .sync_cache()
        .await
        .map_err(|e| {
            tracing::error!("Failed to sync cache: {}", e);
            AppError::internal_error(format!("Failed to sync cache: {}", e))
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
