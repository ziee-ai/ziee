//! whisper-server binary version resolution + readiness.
//!
//! Mirrors `llm_local_runtime::binary_manager` (select_version, check_for_updates,
//! set_system_default, sync_cache), scoped to the single whisper engine. The
//! download + update handlers land in the `runtime_version` layer; this file
//! owns host detection + the readiness check the capability endpoint consumes.

use std::path::PathBuf;

use uuid::Uuid;

use crate::common::AppError;
use crate::modules::llm_local_runtime::utils::gpu_detect;
use crate::modules::voice::engine::{
    WhisperDownloader, asset_size_for_backend, available_backends,
};
use crate::modules::voice::runtime_version::models::{
    AvailableUpdatesResponse, AvailableVersion, RuntimeVersion,
};
use crate::modules::voice::runtime_version::repository;

/// Host platform string (`linux` | `macos` | `windows`) — reuses the LLM
/// runtime's detection so the asset-naming contract matches the fork's CI.
pub fn host_platform() -> String {
    gpu_detect::host_platform()
}

/// Host arch string (`x86_64` | `aarch64`).
pub fn host_arch() -> String {
    gpu_detect::host_arch()
}

/// True when at least one whisper-server binary is installed for THIS host
/// (any backend) — i.e. the runtime can start. The capability endpoint uses
/// this to decide whether the composer mic is usable.
pub async fn runtime_ready() -> bool {
    let platform = host_platform();
    let arch = host_arch();
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM voice_runtime_versions
           WHERE platform = $1 AND arch = $2"#,
        platform,
        arch,
    )
    .fetch_one(crate::core::Repos.pool())
    .await
    .unwrap_or(0);
    count > 0
}

// =====================================================================
// Version selection + management (single-engine BinaryManager analog)
// =====================================================================

/// Select the whisper runtime version the lifecycle layer should start:
/// the system default if one is set, else the latest by `created_at`. Returns
/// `None` when no version is installed at all.
pub async fn select_version() -> Result<Option<RuntimeVersion>, AppError> {
    let pool = crate::core::Repos.pool();
    if let Some(v) = repository::get_system_default(pool)
        .await
        .map_err(AppError::database_error)?
    {
        return Ok(Some(v));
    }
    repository::get_latest_version(pool)
        .await
        .map_err(AppError::database_error)
}

/// Resolve the on-disk `whisper-server` executable path for the selected
/// version. This is the key entry point the lifecycle layer (deployment /
/// auto_start) depends on: it returns the path to spawn, or a clear
/// `not_found` error when no whisper runtime is installed / the cached binary
/// is missing from disk.
pub async fn ensure_binary_path() -> Result<PathBuf, AppError> {
    let version = select_version().await?.ok_or_else(|| {
        AppError::not_found(
            "no whisper runtime installed — install one under /settings (Voice) first",
        )
    })?;

    let path = PathBuf::from(&version.binary_path);
    if !path.exists() {
        return Err(AppError::not_found(&format!(
            "whisper runtime {} is registered but its binary is missing from disk ({})",
            version.version,
            path.display()
        )));
    }
    Ok(path)
}

/// Set a whisper runtime version as the system default (clears any existing
/// default first). 404 when `version_id` does not exist.
pub async fn set_system_default(version_id: Uuid) -> Result<(), AppError> {
    let pool = crate::core::Repos.pool();
    // Verify it exists first so a bad id is a clean 404, not a silent no-op.
    repository::get_by_id(pool, version_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Runtime version"))?;

    repository::clear_system_default(pool)
        .await
        .map_err(AppError::database_error)?;
    repository::set_system_default(pool, version_id, true)
        .await
        .map_err(AppError::database_error)?;
    tracing::info!("Set whisper system default: {}", version_id);
    Ok(())
}

/// Check GitHub for available whisper runtime versions and diff against what is
/// installed, scoped to the detected host platform/arch.
pub async fn check_for_updates() -> Result<AvailableUpdatesResponse, AppError> {
    let platform = host_platform();
    let arch = host_arch();

    let downloader = WhisperDownloader::new().map_err(AppError::internal_with_id)?;
    let releases = downloader
        .list_releases()
        .await
        .map_err(AppError::internal_with_id)?;

    let pool = crate::core::Repos.pool();
    let installed = repository::list_all(pool, 1, 500)
        .await
        .map_err(AppError::database_error)?;

    let versions = releases
        .into_iter()
        .filter(|r| !r.draft)
        .map(|r| {
            let avail = available_backends(&platform, &arch, &r.assets);
            let installed_backends: Vec<String> = installed
                .iter()
                .filter(|v| v.version == r.version && v.platform == platform && v.arch == arch)
                .map(|v| v.backend.clone())
                .collect();
            let recommended_backend = gpu_detect::recommend_backend(&avail);
            let size_bytes = recommended_backend
                .as_deref()
                .or_else(|| avail.first().map(|s| s.as_str()))
                .and_then(|backend| {
                    asset_size_for_backend(&platform, &arch, backend, &r.assets)
                });
            AvailableVersion {
                version: r.version,
                installed: !installed_backends.is_empty(),
                installed_backends,
                binary_ready: !avail.is_empty(),
                available_backends: avail,
                recommended_backend,
                size_bytes,
                prerelease: r.prerelease,
                published_at: r.published_at,
            }
        })
        .collect();

    Ok(AvailableUpdatesResponse {
        platform,
        arch,
        versions,
    })
}

/// Scan the whisper binary cache dir and back-fill DB rows for any cached
/// binary that lacks one. Returns the number of rows created.
pub async fn sync_cache() -> Result<usize, AppError> {
    let downloader = WhisperDownloader::new().map_err(AppError::internal_with_id)?;
    let cached = downloader.list_binaries().map_err(AppError::internal_with_id)?;

    let pool = crate::core::Repos.pool();
    let mut synced = 0usize;
    for b in cached {
        if repository::get_by_identity(pool, &b.version, &b.platform, &b.arch, &b.backend)
            .await
            .map_err(AppError::database_error)?
            .is_some()
        {
            continue;
        }
        repository::create(
            pool,
            &b.version,
            &b.platform,
            &b.arch,
            &b.backend,
            b.path.to_string_lossy().as_ref(),
        )
        .await
        .map_err(AppError::database_error)?;
        tracing::info!("Synced cached whisper binary to database: {}", b.version);
        synced += 1;
    }
    Ok(synced)
}
