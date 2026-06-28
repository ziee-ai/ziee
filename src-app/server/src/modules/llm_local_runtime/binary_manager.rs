//! Binary Manager - Manages runtime binary versions and downloads
//!
//! Wraps the llm-runtime crate's BinaryDownloader to provide:
//! - Database-backed version tracking
//! - Download and registration workflow
//! - Version lookup with fallback logic
//! - Cached binary path resolution

use sqlx::PgPool;
use crate::modules::llm_local_runtime::runtime_version::repository as version_repo;
use crate::modules::llm_local_runtime::runtime_version::models::{AvailableVersion, RuntimeVersion};
use crate::modules::llm_local_runtime::engine::{
    asset_size_for_backend, available_backends, BinaryDownloader, EngineType,
};
use std::path::PathBuf;
use uuid::Uuid;

/// Manages runtime binary versions and downloads
pub struct BinaryManager {
    downloader: BinaryDownloader,
    pool: PgPool,
}

impl BinaryManager {
    /// Create a new binary manager with default cache directory
    ///
    /// NOTE: dead_code allowed — only `with_cache_dir` is used in
    /// production (the config-driven path). `new()` exists for
    /// convenience in tests / future callers that want the default.
    #[allow(dead_code)]
    pub fn new(pool: PgPool) -> Result<Self, Box<dyn std::error::Error>> {
        let downloader = BinaryDownloader::new()?;
        Ok(Self { downloader, pool })
    }

    /// Create a binary manager with custom cache directory
    pub fn with_cache_dir(
        pool: PgPool,
        cache_dir: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let downloader = BinaryDownloader::with_binaries_dir(cache_dir)?;
        Ok(Self { downloader, pool })
    }

    /// Download and register a new runtime version (no progress
    /// reporting). Thin wrapper around
    /// [`Self::download_and_register_with_progress`].
    ///
    /// NOTE: dead_code allowed — production callers use
    /// `download_and_register_with_progress` directly for progress.
    #[allow(dead_code)]
    pub async fn download_and_register(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
    ) -> Result<RuntimeVersion, Box<dyn std::error::Error + Send + Sync>> {
        self.download_and_register_with_progress(
            engine,
            version,
            platform,
            arch,
            backend,
            |_, _| {},
        )
        .await
    }

    /// Download and register a new runtime version, calling `progress`
    /// on every chunk read with `(bytes_received, total_bytes)`.
    /// `total_bytes` is `None` when the upstream omits Content-Length.
    /// If the binary is already cached, skips the download (and the
    /// callback) but still creates the DB record.
    ///
    /// The `Send + Sync` bounds on the error type let this be called
    /// from inside a `tokio::spawn`'d future (the detached
    /// download-task runner).
    pub async fn download_and_register_with_progress<F>(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
        progress: F,
    ) -> Result<RuntimeVersion, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(u64, Option<u64>) + Send + Sync,
    {
        // Download binary (uses cache if available)
        let binary_info = self
            .downloader
            .download_with_progress(engine, version, platform, arch, backend, progress)
            .await?;

        // Check if version already exists in database
        let engine_name = match engine {
            EngineType::Llamacpp => "llamacpp",
            EngineType::Mistralrs => "mistralrs",
        };

        if let Some(existing) = version_repo::get_by_engine_and_version(
            &self.pool,
            engine_name,
            &binary_info.version,
        )
        .await?
        {
            tracing::info!(
                "Runtime version already registered: {} {}",
                engine_name,
                binary_info.version
            );
            return Ok(existing);
        }

        // Create database record
        let runtime_version = version_repo::create(
            &self.pool,
            engine_name,
            &binary_info.version,
            platform,
            arch,
            backend,
            binary_info.path.to_string_lossy().as_ref(),
        )
        .await?;

        tracing::info!(
            "Registered runtime version: {} {} ({})",
            engine_name,
            binary_info.version,
            runtime_version.id
        );

        Ok(runtime_version)
    }

    /// Get binary path for a specific runtime version
    ///
    /// First checks the database for the version, then verifies the binary exists on disk.
    pub async fn get_binary_path(
        &self,
        version_id: Uuid,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let version = version_repo::get_by_id(&self.pool, version_id)
            .await?
            .ok_or("Runtime version not found")?;

        let binary_path = PathBuf::from(&version.binary_path);

        if !binary_path.exists() {
            return Err(format!(
                "Binary not found at path: {}",
                binary_path.display()
            )
            .into());
        }

        Ok(binary_path)
    }

    /// Get binary path by engine and version string
    ///
    /// NOTE: dead_code allowed — callers use `get_binary_path(version_id)`
    /// after resolving the version through other means.
    #[allow(dead_code)]
    pub async fn get_binary_path_by_version(
        &self,
        engine: &str,
        version: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let runtime_version = version_repo::get_by_engine_and_version(&self.pool, engine, version)
            .await?
            .ok_or(format!("Runtime version not found: {} {}", engine, version))?;

        self.get_binary_path(runtime_version.id).await
    }

    /// List all registered runtime versions from database
    pub async fn list_versions(&self) -> Result<Vec<RuntimeVersion>, Box<dyn std::error::Error>> {
        Ok(version_repo::list_all(&self.pool).await?)
    }

    /// List versions for a specific engine
    pub async fn list_versions_for_engine(
        &self,
        engine: &str,
    ) -> Result<Vec<RuntimeVersion>, Box<dyn std::error::Error>> {
        Ok(version_repo::list_by_engine(&self.pool, engine).await?)
    }

    /// Get the latest version for an engine
    pub async fn get_latest_version(
        &self,
        engine: &str,
    ) -> Result<Option<RuntimeVersion>, Box<dyn std::error::Error>> {
        Ok(version_repo::get_latest_version(&self.pool, engine).await?)
    }

    /// Set a version as the system default
    pub async fn set_system_default(
        &self,
        version_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let version = version_repo::get_by_id(&self.pool, version_id)
            .await?
            .ok_or("Runtime version not found")?;

        // Clear existing default for this engine
        version_repo::clear_system_default(&self.pool, &version.engine).await?;

        // Set new default
        version_repo::set_system_default(&self.pool, version_id, true).await?;

        tracing::info!(
            "Set system default: {} {} ({})",
            version.engine,
            version.version,
            version_id
        );

        Ok(())
    }

    /// Get the system default version for an engine
    pub async fn get_system_default(
        &self,
        engine: &str,
    ) -> Result<Option<RuntimeVersion>, Box<dyn std::error::Error>> {
        Ok(version_repo::get_system_default(&self.pool, engine).await?)
    }

    /// Delete a runtime version from database and optionally remove binary
    pub async fn delete_version(
        &self,
        version_id: Uuid,
        remove_binary: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let version = version_repo::get_by_id(&self.pool, version_id)
            .await?
            .ok_or("Runtime version not found")?;

        // Check if version is in use (this would need to check against models/instances)
        // For now, we'll skip this check

        // Remove binary file if requested
        if remove_binary {
            let binary_path = PathBuf::from(&version.binary_path);
            if binary_path.exists() {
                // Also remove the containing directory (includes shared libraries)
                if let Some(parent) = binary_path.parent() {
                    std::fs::remove_dir_all(parent)?;
                    tracing::info!("Removed binary directory: {}", parent.display());
                }
            }
        }

        // Delete from database
        version_repo::delete(&self.pool, version_id).await?;

        tracing::info!(
            "Deleted runtime version: {} {} ({})",
            version.engine,
            version.version,
            version_id
        );

        Ok(())
    }

    /// Check for available versions upstream and diff against what is
    /// installed, scoped to a host (`platform`, `arch`).
    ///
    /// Release discovery is delegated to [`BinaryDownloader::list_releases`]
    /// so it is **mirror-aware** (honours `LLM_RUNTIME_API_MIRROR`, unlike
    /// the previous hardcoded `api.github.com` call) and shares the engine
    /// archive-naming with the download path.
    ///
    /// Per release we compute:
    /// - `available_backends` — backends whose binary is published upstream
    ///   for this host. Empty ⇒ the tag exists but the build is pending
    ///   (`binary_ready=false`), so it is surfaced but not installable.
    /// - `installed_backends` — backends already in the DB for this
    ///   version + host platform/arch.
    ///
    /// Drafts are omitted (not public). Prereleases are kept but flagged.
    pub async fn check_for_updates(
        &self,
        engine: &str,
        platform: &str,
        arch: &str,
    ) -> Result<Vec<AvailableVersion>, Box<dyn std::error::Error>> {
        let engine_type = match engine {
            "llamacpp" => EngineType::Llamacpp,
            "mistralrs" => EngineType::Mistralrs,
            _ => return Err(format!("Unknown engine: {}", engine).into()),
        };

        let releases = self.downloader.list_releases(engine_type).await?;
        let installed = version_repo::list_by_engine(&self.pool, engine).await?;

        let versions = releases
            .into_iter()
            .filter(|r| !r.draft)
            .map(|r| {
                let avail = available_backends(engine_type, platform, arch, &r.assets);
                let installed_backends: Vec<String> = installed
                    .iter()
                    .filter(|v| {
                        v.version == r.version && v.platform == platform && v.arch == arch
                    })
                    .map(|v| v.backend.clone())
                    .collect();
                let recommended_backend =
                    crate::modules::llm_local_runtime::utils::gpu_detect::recommend_backend(&avail);
                // Size of the asset the inline Install button would
                // actually fetch: the recommended backend when known,
                // else the first published backend (matches the
                // fallback in the UI's `handleDownload`).
                let size_bytes = recommended_backend
                    .as_deref()
                    .or_else(|| avail.first().map(|s| s.as_str()))
                    .and_then(|backend| {
                        asset_size_for_backend(
                            engine_type,
                            platform,
                            arch,
                            backend,
                            &r.assets,
                        )
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

        Ok(versions)
    }

    /// Sync database with cached binaries
    ///
    /// Scans the cache directory and ensures all cached binaries have database records.
    /// This is useful for initial setup or after manual cache manipulation.
    pub async fn sync_cache(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let cached_binaries = self.downloader.list_binaries()?;
        let mut synced_count = 0;

        for binary in cached_binaries {
            let engine_name = match binary.engine {
                EngineType::Llamacpp => "llamacpp",
                EngineType::Mistralrs => "mistralrs",
            };

            // Check if already in database
            if version_repo::get_by_engine_and_version(
                &self.pool,
                engine_name,
                &binary.version,
            )
            .await?
            .is_some()
            {
                continue;
            }

            // Create database record
            version_repo::create(
                &self.pool,
                engine_name,
                &binary.version,
                &binary.platform,
                &binary.arch,
                &binary.backend,
                binary.path.to_string_lossy().as_ref(),
            )
            .await?;

            tracing::info!(
                "Synced cached binary to database: {} {}",
                engine_name,
                binary.version
            );

            synced_count += 1;
        }

        Ok(synced_count)
    }

    /// Get the cache directory path
    ///
    /// NOTE: dead_code allowed — the downloader owns the cache dir;
    /// callers that need it coordinate via config, not this accessor.
    #[allow(dead_code)]
    pub fn cache_dir(&self) -> &std::path::Path {
        self.downloader.binaries_dir()
    }

    /// Select appropriate runtime version using fallback chain:
    /// 1. Model's required_runtime_version_id (if model_id provided)
    /// 2. Provider's default_runtime_version_id (if provider_id provided)
    /// 3. System default for engine
    /// 4. Latest version for engine
    ///
    /// Returns None if no version is available at any level.
    pub async fn select_runtime_version(
        &self,
        model_id: Option<Uuid>,
        provider_id: Option<Uuid>,
        engine: &str,
    ) -> Result<Option<RuntimeVersion>, Box<dyn std::error::Error>> {
        // Step 1: Check model's required version
        if let Some(mid) = model_id {
            let model_version = sqlx::query!(
                "SELECT required_runtime_version_id FROM llm_models WHERE id = $1",
                mid
            )
            .fetch_optional(&self.pool)
            .await?;

            if let Some(record) = model_version
                && let Some(version_id) = record.required_runtime_version_id
                    && let Some(version) = version_repo::get_by_id(&self.pool, version_id).await? {
                        tracing::debug!(
                            "Selected runtime version from model: {} {}",
                            version.engine,
                            version.version
                        );
                        return Ok(Some(version));
                    }
        }

        // Step 2: Check provider's default version
        if let Some(pid) = provider_id {
            let provider_version = sqlx::query!(
                "SELECT default_runtime_version_id FROM llm_providers WHERE id = $1",
                pid
            )
            .fetch_optional(&self.pool)
            .await?;

            if let Some(record) = provider_version
                && let Some(version_id) = record.default_runtime_version_id
                    && let Some(version) = version_repo::get_by_id(&self.pool, version_id).await? {
                        tracing::debug!(
                            "Selected runtime version from provider: {} {}",
                            version.engine,
                            version.version
                        );
                        return Ok(Some(version));
                    }
        }

        // Step 3: Check system default
        if let Some(version) = self.get_system_default(engine).await? {
            tracing::debug!(
                "Selected runtime version from system default: {} {}",
                version.engine,
                version.version
            );
            return Ok(Some(version));
        }

        // Step 4: Use latest version
        if let Some(version) = self.get_latest_version(engine).await? {
            tracing::debug!(
                "Selected runtime version from latest: {} {}",
                version.engine,
                version.version
            );
            return Ok(Some(version));
        }

        tracing::warn!("No runtime version available for engine: {}", engine);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    

    // Tests would go here, but require database setup
    // These would be integration tests in tests/integration_tests/
}
