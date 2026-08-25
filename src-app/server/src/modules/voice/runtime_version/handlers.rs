//! Admin REST handlers for whisper runtime version management.
//!
//! Single-engine analog of `llm_local_runtime::runtime_version::handlers`,
//! gated by the voice admin permission split (`voice::admin::{read,manage}`).

use aide::axum::IntoApiResponse;
use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

use crate::modules::voice::binary_manager;
use crate::modules::voice::permissions::{VoiceAdminManage, VoiceAdminRead};

use super::download_task::{
    self, DOWNLOAD_TASKS, DownloadTask, SSEEngineDownloadConnectedData, SSEEngineDownloadEvent,
};
use super::models::*;
use super::repository;

// =====================================================
// Query parameters + validators
// =====================================================

const DEFAULT_PAGE_SIZE: i64 = 500;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListVersionsQuery {
    /// Page number (1-indexed, default 1).
    #[serde(default = "default_page")]
    pub page: i64,
    /// Items per page (default 500, max 500).
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    DEFAULT_PAGE_SIZE
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteVersionQuery {
    /// Whether to remove the binary file from disk (defaults to false).
    pub remove_binary: Option<bool>,
}

/// A backend artifact tag: starts with a lowercase letter, then `[a-z0-9.]`.
/// Rejects `..` / path separators.
fn is_valid_backend(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
        && !s.contains("..")
}

/// A release tag: starts alphanumeric, then `[A-Za-z0-9._-]`. Rejects `..`,
/// `/`, leading `-`.
fn is_valid_release_tag(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !s.contains("..")
}

// =====================================================
// Handlers
// =====================================================

/// List all registered whisper runtime versions.
pub async fn list_versions(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Query(params): Query<ListVersionsQuery>,
) -> ApiResult<Json<RuntimeVersionListResponse>> {
    let pool = Repos.pool();
    let versions = repository::list_all(pool, params.page, params.per_page)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?;
    let response = RuntimeVersionListResponse {
        versions: versions.into_iter().map(RuntimeVersionResponse::from).collect(),
    };
    Ok((StatusCode::OK, Json(response)))
}

/// Check for available whisper runtime updates from GitHub, scoped to the host.
pub async fn check_updates(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<AvailableUpdatesResponse>> {
    let response = binary_manager::check_for_updates()
        .await
        .map_err(|e| e.to_api_error())?;
    Ok((StatusCode::OK, Json(response)))
}

/// Start (or join) a detached download of a whisper runtime version.
pub async fn download_version(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Json(req): Json<DownloadVersionRequest>,
) -> ApiResult<Json<DownloadVersionStartedResponse>> {
    // Resolve host defaults for the optional fields.
    let platform = req.platform.unwrap_or_else(binary_manager::host_platform);
    let arch = req.arch.unwrap_or_else(binary_manager::host_arch);
    let backend = req.backend.unwrap_or_else(|| "cpu".to_string());

    // Validate BEFORE the values reach the cache path + download URL — a `../`
    // in version/platform would traverse out of the cache dir, and the resolved
    // binary_path is later spawned as a child process.
    if !matches!(platform.as_str(), "linux" | "macos" | "windows") {
        return Err(AppError::bad_request(
            "INVALID_PLATFORM",
            "Platform must be linux, macos, or windows",
        )
        .to_api_error());
    }
    if !matches!(arch.as_str(), "x86_64" | "aarch64") {
        return Err(AppError::bad_request(
            "INVALID_ARCH",
            "Architecture must be x86_64 or aarch64",
        )
        .to_api_error());
    }
    if !is_valid_backend(&backend) {
        return Err(AppError::bad_request(
            "INVALID_BACKEND",
            "Backend must start with a lowercase letter and contain only [a-z0-9.] (e.g. cpu, cuda12.6)",
        )
        .to_api_error());
    }
    if !is_valid_release_tag(&req.version) {
        return Err(AppError::bad_request(
            "INVALID_VERSION",
            "Version must be a release tag of [A-Za-z0-9._-] (e.g. v0.0.1-alpha, latest) with no path separators",
        )
        .to_api_error());
    }

    let task = download_task::start_or_join(req.version.clone(), platform, arch, backend)
        .await
        .map_err(|e| e.to_api_error())?;

    let status = format!("{:?}", task.state.lock().await.status).to_lowercase();
    let response = DownloadVersionStartedResponse {
        task_id: task.task_id,
        key: task.key.clone(),
        version: task.version.clone(),
        backend: task.backend.clone(),
        status,
        // `@` is a valid URL path char (RFC 3986 pchar); version/backend
        // validation rejects `/` + `..`, so the raw key is safe to inline.
        events_url: format!("/api/voice/versions/downloads/{}/events", task.key),
    };

    // NOTE: the VoiceRuntimeVersion Create sync event is emitted from the
    // detached download task on COMPLETION — the row doesn't exist yet here.
    Ok((StatusCode::OK, Json(response)))
}

/// Build a `DownloadSnapshot` from a registry entry.
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

/// List in-flight download tasks (terminal entries excluded) so a page reload
/// repaints only active progress.
pub async fn list_active_downloads(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<DownloadListResponse>> {
    let entries: Vec<Arc<DownloadTask>> =
        DOWNLOAD_TASKS.iter().map(|e| e.value().clone()).collect();
    let mut downloads = Vec::with_capacity(entries.len());
    for t in entries {
        let snap = snapshot_of(&t).await;
        if matches!(snap.status.as_str(), "completed" | "failed") {
            continue;
        }
        downloads.push(snap);
    }
    downloads.sort_by(|a, b| a.key.cmp(&b.key));
    Ok((StatusCode::OK, Json(DownloadListResponse { downloads })))
}

/// Single-download poll snapshot (non-SSE fallback; F9 parity with the llm runtime).
pub async fn get_download_snapshot(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Path(key): Path<String>,
) -> ApiResult<Json<DownloadSnapshot>> {
    let task = download_task::get_task(&key)
        .ok_or_else(|| AppError::not_found(&format!("download {key:?}")).to_api_error())?;
    Ok((StatusCode::OK, Json(snapshot_of(&task).await)))
}

pub fn get_download_snapshot_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getVersionDownload")
        .tag("Voice")
        .summary("Poll a single whisper runtime-version download snapshot")
        .response::<200, Json<DownloadSnapshot>>()
}

/// Fetch a single runtime version by id (F10 parity).
pub async fn get_version(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Path(version_id): Path<Uuid>,
) -> ApiResult<Json<RuntimeVersion>> {
    let pool = Repos.pool();
    let v = repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("voice runtime version").to_api_error())?;
    Ok((StatusCode::OK, Json(v)))
}

pub fn get_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getVersion")
        .tag("Voice")
        .summary("Read a single whisper runtime version")
        .response::<200, Json<RuntimeVersion>>()
}

// NOTE: a standalone `GET /voice/detect-gpu` (F10) is intentionally NOT added —
// the backend recommendation is already surfaced by `check-updates`
// (`binary_manager` folds `gpu_detect::recommend_backend` into it), and a proper
// available-backends list requires an upstream release-asset fetch. Descoped as
// redundant (see DECISIONS DEC-19); not a silent omission.

/// SSE stream of download events for a single task.
pub async fn subscribe_download_events(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    Path(key): Path<String>,
) -> ApiResult<
    axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>>,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let task = download_task::get_task(&key)
        .ok_or_else(|| AppError::not_found(&format!("download task {key:?}")).to_api_error())?;

    // Subscribe BEFORE snapshotting so no event is dropped in between.
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
        let failed = snap
            .error
            .as_ref()
            .map(|e| super::download_task::SSEEngineDownloadFailedData { error: e.clone() });
        (snap.status, replay, complete, failed)
    };

    let task_clone = task.clone();
    let stream = async_stream::stream! {
        yield Ok::<Event, axum::Error>(SSEEngineDownloadEvent::Connected(SSEEngineDownloadConnectedData {
            task_id: task_clone.task_id,
            key: task_clone.key.clone(),
            version: task_clone.version.clone(),
            backend: task_clone.backend.clone(),
            status: initial_status,
        }).into());

        for p in replay {
            yield Ok(SSEEngineDownloadEvent::Progress(p).into());
        }

        if let Some(c) = terminal_complete {
            yield Ok(SSEEngineDownloadEvent::Complete(c).into());
            return;
        }
        if let Some(f) = terminal_failed {
            yield Ok(SSEEngineDownloadEvent::Failed(f).into());
            return;
        }

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

/// Delete a whisper runtime version. Refuses (409) when the version is the
/// system default or is referenced by the managed instance.
pub async fn delete_version(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Path(version_id): Path<Uuid>,
    Query(params): Query<DeleteVersionQuery>,
    origin: SyncOrigin,
) -> ApiResult<impl IntoApiResponse> {
    let pool = Repos.pool();
    let remove_binary = params.remove_binary.unwrap_or(false);

    // Idempotent: already gone → 204.
    let version_record = match repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?
    {
        Some(v) => v,
        None => return Ok((StatusCode::NO_CONTENT, ())),
    };

    // In-use guard. The instance FK is ON DELETE SET NULL, so the DB would
    // silently orphan the singleton (breaking auto-start) rather than erroring.
    let usage = repository::usage(pool, version_id)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?;
    if version_record.is_system_default
        || usage.running_instances > 0
        || usage.referencing_instances > 0
    {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "VERSION_IN_USE",
            format!(
                "Cannot delete: this whisper version is the system default \
                 ({}) or is referenced by the managed instance ({} running, \
                 {} bound). Set a different default (or stop/rebind the \
                 instance) first.",
                version_record.is_system_default,
                usage.running_instances,
                usage.referencing_instances
            ),
        )
        .to_api_error());
    }

    // Remove the binary directory if requested.
    if remove_binary {
        let binary_path = std::path::PathBuf::from(&version_record.binary_path);
        if let Some(parent) = binary_path.parent() {
            if parent.exists() {
                if let Err(e) = std::fs::remove_dir_all(parent) {
                    tracing::warn!("Failed to remove whisper binary dir {}: {e}", parent.display());
                } else {
                    tracing::info!("Removed whisper binary directory: {}", parent.display());
                }
            }
        }
    }

    repository::delete(pool, version_id)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?;

    sync_publish(
        SyncEntity::VoiceRuntimeVersion,
        SyncAction::Delete,
        version_id,
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, ()))
}

/// Set a whisper runtime version as the system default.
pub async fn set_default(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    Path(version_id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<Json<RuntimeVersionResponse>> {
    binary_manager::set_system_default(version_id)
        .await
        .map_err(|e| e.to_api_error())?;

    let pool = Repos.pool();
    let version_record = repository::get_by_id(pool, version_id)
        .await
        .map_err(|e| AppError::database_error(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("Runtime version").to_api_error())?;

    sync_publish(
        SyncEntity::VoiceRuntimeVersion,
        SyncAction::Update,
        version_id,
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(RuntimeVersionResponse::from(version_record))))
}

// =====================================================
// OpenAPI describers
// =====================================================

pub fn list_versions_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.listVersions")
        .description("List all registered whisper runtime versions")
        .tag("Voice")
        .response::<200, Json<RuntimeVersionListResponse>>()
}

pub fn check_updates_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.checkVersionUpdates")
        .description("Check for available whisper runtime version updates from GitHub, scoped to the host")
        .tag("Voice")
        .response::<200, Json<AvailableUpdatesResponse>>()
}

pub fn download_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.downloadVersion")
        .description(
            "Start (or join) a detached download of a whisper runtime version. \
             Returns immediately with task identifiers + an SSE URL; the \
             download keeps running on the server even after the client \
             disconnects, so a page reload can pick it up via \
             GET /voice/versions/downloads.",
        )
        .tag("Voice")
        .response::<200, Json<DownloadVersionStartedResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request parameters"))
}

pub fn list_active_downloads_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.listVersionDownloads")
        .description("List in-flight whisper download tasks (for repainting progress after a page reload)")
        .tag("Voice")
        .response::<200, Json<DownloadListResponse>>()
}

pub fn subscribe_download_events_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.subscribeVersionDownloadEvents")
        .description(
            "Subscribe to SSE progress events for one whisper download task. \
             Sends Connected with the current snapshot immediately, replays \
             buffered Progress frames, then live-streams until Complete/Failed.",
        )
        .tag("Voice")
        .response::<200, Json<SSEEngineDownloadEvent>>()
        .response_with::<404, (), _>(|res| res.description("No such download task"))
}

pub fn delete_version_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.deleteVersion")
        .description("Delete a whisper runtime version (409 when it is the default or in use by the instance)")
        .tag("Voice")
        .response_with::<204, (), _>(|res| res.description("Runtime version deleted successfully"))
        .response_with::<409, (), _>(|res| res.description("Version is in use / is the system default"))
}

pub fn set_default_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.setDefaultVersion")
        .description("Set a whisper runtime version as the system default")
        .tag("Voice")
        .response::<200, Json<RuntimeVersionResponse>>()
        .response_with::<404, (), _>(|res| res.description("Runtime version not found"))
}
