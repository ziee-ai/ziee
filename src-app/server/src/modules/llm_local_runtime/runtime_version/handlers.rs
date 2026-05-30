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
use super::download_task::{
    self, DOWNLOAD_TASKS, DownloadTask, SSEEngineDownloadConnectedData,
    SSEEngineDownloadEvent,
};

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

/// Start (or join) a download task for a runtime version.
///
/// Returns immediately with the task identifier and an SSE URL the
/// client subscribes to for live progress; the actual download runs
/// detached on the server so the client can disconnect / reload the
/// page without aborting it. Re-POSTs for an already-running
/// (engine, version, backend) triple join the existing task instead
/// of spawning a duplicate (DashMap entry-locked).
pub async fn download_runtime_version(
    _auth: RequirePermissions<(RuntimeVersionCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(req): Json<DownloadVersionRequest>,
) -> ApiResult<Json<DownloadVersionStartedResponse>> {
    let pool = Repos.pool();
    let binary_manager = BinaryManager::with_cache_dir(
        pool.clone(),
        std::path::PathBuf::from(crate::core::get_caches_config().llm_engines_dir()),
    )
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

    // Kick off (or join) the detached download task. The task itself
    // emits the cache-invalidation event when it finishes; we don't
    // wait here.
    let binary_manager = Arc::new(binary_manager);
    let task = download_task::start_or_join(
        binary_manager,
        engine,
        req.version.clone(),
        req.platform.clone(),
        req.arch.clone(),
        req.backend.clone(),
    )
    .await
    .map_err(|e| e.to_api_error())?;

    // Fire the "started" cache invalidation now so any UI that
    // listens for runtime_version events refreshes its in-flight
    // state immediately. The "completed" event is fired from the
    // task runner once the binary is registered (see download_task).
    let _ = event_bus; // reserved for a future started-event variant

    let status = format!("{:?}", task.state.lock().await.status).to_lowercase();
    let response = DownloadVersionStartedResponse {
        task_id: task.task_id,
        key: task.key.clone(),
        engine: task.engine.clone(),
        version: task.version.clone(),
        backend: task.backend.clone(),
        status,
        // `@` is a valid URL path char (RFC 3986 pchar), and our
        // version/backend validation rejects `/` + `..`, so the raw
        // key is safe to inline without percent-encoding.
        events_url: format!(
            "/api/local-runtime/versions/downloads/{}/events",
            task.key
        ),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Build a `DownloadSnapshot` from a registry entry. Async because
/// it locks the per-task state mutex.
async fn snapshot_of(task: &Arc<DownloadTask>) -> DownloadSnapshot {
    let guard = task.state.lock().await;
    let percent = guard.total_bytes.map(|t| {
        if t == 0 {
            0.0
        } else {
            (guard.bytes_received as f32 / t as f32) * 100.0
        }
    });
    DownloadSnapshot {
        task_id: task.task_id,
        key: task.key.clone(),
        engine: task.engine.clone(),
        version: task.version.clone(),
        backend: task.backend.clone(),
        status: format!("{:?}", guard.status).to_lowercase(),
        bytes_received: guard.bytes_received,
        total_bytes: guard.total_bytes,
        percent,
        result_version_id: guard.result.as_ref().map(|v| v.id),
        error: guard.error.clone(),
    }
}

/// List every download task in the in-process registry — running OR
/// terminal-but-not-yet-replaced. The UI calls this on mount to
/// repaint in-flight progress after a page reload.
pub async fn list_active_downloads(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
) -> ApiResult<Json<DownloadListResponse>> {
    let entries: Vec<Arc<DownloadTask>> =
        DOWNLOAD_TASKS.iter().map(|e| e.value().clone()).collect();
    let mut downloads = Vec::with_capacity(entries.len());
    for t in entries {
        downloads.push(snapshot_of(&t).await);
    }
    // Stable order so the UI doesn't reshuffle on every poll/reload.
    downloads.sort_by(|a, b| a.key.cmp(&b.key));
    Ok((StatusCode::OK, Json(DownloadListResponse { downloads })))
}

/// Snapshot of a single download task. 404 when the key isn't in the
/// registry. Used by tests + the UI as a fallback poll if SSE is
/// unavailable.
pub async fn get_download_snapshot(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Path(key): Path<String>,
) -> ApiResult<Json<DownloadSnapshot>> {
    let task = download_task::get_task(&key).ok_or_else(|| {
        AppError::not_found(&format!("download task {key:?}"))
    })?;
    Ok((StatusCode::OK, Json(snapshot_of(&task).await)))
}

/// SSE stream of download events for a single task. Sends Connected
/// (with the current state snapshot) immediately, replays buffered
/// Progress frames, then live-streams further events until the task
/// reaches a terminal state and the broadcast closes. Late
/// subscribers to an already-terminal task see Connected + the
/// terminal event + close.
pub async fn subscribe_download_events(
    _auth: RequirePermissions<(RuntimeVersionRead,)>,
    Path(key): Path<String>,
) -> ApiResult<
    axum::response::Sse<
        impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>,
    >,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let task = download_task::get_task(&key).ok_or_else(|| {
        AppError::not_found(&format!("download task {key:?}"))
    })?;

    // Subscribe BEFORE snapshotting to avoid dropping events that
    // arrive between the snapshot read and the .subscribe() call.
    let mut rx = task.events.subscribe();
    let (initial_status, replay, terminal_complete, terminal_failed) = {
        let snap = task.state.lock().await;
        let percent = snap.total_bytes.map(|t| {
            if t == 0 {
                0.0
            } else {
                (snap.bytes_received as f32 / t as f32) * 100.0
            }
        });
        // If we hold a "current" frame that didn't make the replay
        // buffer (cap eviction), seed it as the first replay so a
        // late subscriber sees the latest bytes immediately.
        let mut replay = snap.progress.clone();
        if replay
            .last()
            .map(|p| p.bytes_received != snap.bytes_received)
            .unwrap_or(true)
        {
            replay.push(super::download_task::SSEEngineDownloadProgressData {
                status: snap.status,
                bytes_received: snap.bytes_received,
                total_bytes: snap.total_bytes,
                percent,
            });
        }
        let complete = snap.result.as_ref().map(|v| {
            super::download_task::SSEEngineDownloadCompleteData {
                version_id: v.id,
                bytes_downloaded: snap.bytes_received,
                duration_ms: 0,
            }
        });
        let failed = snap.error.as_ref().map(|e| {
            super::download_task::SSEEngineDownloadFailedData { error: e.clone() }
        });
        (snap.status, replay, complete, failed)
    };

    let task_clone = task.clone();
    let stream = async_stream::stream! {
        // 1. Connected — first event for every subscriber.
        yield Ok::<Event, axum::Error>(SSEEngineDownloadEvent::Connected(SSEEngineDownloadConnectedData {
            task_id: task_clone.task_id,
            key: task_clone.key.clone(),
            engine: task_clone.engine.clone(),
            version: task_clone.version.clone(),
            backend: task_clone.backend.clone(),
            status: initial_status,
        }).into());

        // 2. Replay buffered progress so a late subscriber paints
        //    the current bar position without waiting for the next
        //    chunk.
        for p in replay {
            yield Ok(SSEEngineDownloadEvent::Progress(p).into());
        }

        // 3. If already terminal, emit the final event + close.
        if let Some(c) = terminal_complete {
            yield Ok(SSEEngineDownloadEvent::Complete(c).into());
            return;
        }
        if let Some(f) = terminal_failed {
            yield Ok(SSEEngineDownloadEvent::Failed(f).into());
            return;
        }

        // 4. Stream live events until the broadcast closes.
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let is_terminal = matches!(
                        ev,
                        SSEEngineDownloadEvent::Complete(_) | SSEEngineDownloadEvent::Failed(_)
                    );
                    yield Ok(ev.into());
                    if is_terminal { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };

    Ok((
        StatusCode::OK,
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
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
        .description(
            "Start (or join) a detached download of a runtime version. \
             Returns immediately with task identifiers + an SSE URL; the \
             download keeps running on the server even after the client \
             disconnects, so a page reload can pick it up via \
             GET /local-runtime/versions/downloads."
        )
        .tag("Runtime Versions")
        .response::<200, Json<DownloadVersionStartedResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description("Invalid request parameters")
        })
}

pub fn list_active_downloads_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.listDownloads")
        .description(
            "List every download task currently held by the in-process \
             registry (running OR terminal-but-not-yet-replaced). The UI \
             calls this on mount to repaint in-flight progress after a \
             page reload."
        )
        .tag("Runtime Versions")
        .response::<200, Json<DownloadListResponse>>()
}

pub fn get_download_snapshot_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.getDownload")
        .description("Snapshot of a single download task by its composite key.")
        .tag("Runtime Versions")
        .response::<200, Json<DownloadSnapshot>>()
        .response_with::<404, (), _>(|res| res.description("No such download task"))
}

pub fn subscribe_download_events_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(RuntimeVersionRead,)>(op)
        .id("RuntimeVersion.subscribeDownloadEvents")
        .description(
            "Subscribe to SSE progress events for one download task. \
             Sends Connected with the current state snapshot immediately, \
             replays buffered Progress frames, then live-streams further \
             events until Complete/Failed."
        )
        .tag("Runtime Versions")
        .response::<200, Json<SSEEngineDownloadEvent>>()
        .response_with::<404, (), _>(|res| res.description("No such download task"))
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
