//! ziee-server implementations of the three sandbox-engine provider seams
//! (`ziee_sandbox::provider`). Built at `mod.rs::init` and stored on
//! `CodeSandboxState`, they delegate back to the retained DB/HTTP halves
//! (`runtime_mount` / `runtime_fetch` / `repository` / `embedded` /
//! `wsl2_agent_embedded`) — keeping the engine crate free of every ziee DB /
//! app-module symbol while preserving the exact runtime behavior.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::common::AppError;
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::repository::CodeSandboxRepository;
use crate::modules::code_sandbox::resource_limits::CodeSandboxResourceLimits;
use crate::modules::code_sandbox::types::HostCapabilities;
use crate::modules::code_sandbox::{runtime_fetch, runtime_mount};

use ziee_sandbox::provider::{
    EnsureOutcome, EvictOutcome, Extracted, FetchError, FetchOutcome, FetchProgress,
    GuestAgentProvider, ResourceLimitsProvider, RootfsProvider, RootfsFormat,
};

/// Rootfs lifecycle provider. Holds what the (now explicit-field) `runtime_mount`
/// / `runtime_fetch` functions need — the DB pool the engine state no longer
/// carries, plus the boot-probed host caps + resolved config.
pub struct ZieeRootfsProvider {
    pub pool: Arc<PgPool>,
    pub host_caps: HostCapabilities,
    pub config: CodeSandboxConfig,
}

#[async_trait]
impl RootfsProvider for ZieeRootfsProvider {
    async fn ensure_rootfs_ready(&self, flavor: &str) -> Result<EnsureOutcome, AppError> {
        runtime_mount::ensure_rootfs_ready(&self.pool, &self.config, &self.host_caps, flavor).await
    }

    fn cache_dir(&self) -> PathBuf {
        runtime_mount::cache_dir(&self.config)
    }

    async fn evict_by_version_flavor(
        &self,
        version_cache_dir: &Path,
        version: &str,
        flavor: &str,
    ) -> EvictOutcome {
        runtime_mount::evict_by_version_flavor(version_cache_dir, version, flavor).await
    }

    async fn ensure_fetched(
        &self,
        cache_dir: &Path,
        flavor: &str,
        progress: Box<dyn Fn(FetchProgress) + Send + Sync>,
    ) -> Result<FetchOutcome, FetchError> {
        runtime_fetch::ensure_fetched(&self.pool, cache_dir, flavor, progress).await
    }

    async fn ensure_fetched_format(
        &self,
        cache_dir: &Path,
        flavor: &str,
        format: RootfsFormat,
        progress: Box<dyn Fn(FetchProgress) + Send + Sync>,
    ) -> Result<FetchOutcome, FetchError> {
        runtime_fetch::ensure_fetched_format(&self.pool, cache_dir, flavor, format, progress).await
    }

    async fn shutdown(&self) {
        runtime_mount::shutdown().await
    }
}

/// Resource-limits DB read behind `resource_limits_cache::get`. The two SQL
/// methods live on the retained `CodeSandboxRepository`.
pub struct ZieeResourceLimitsProvider {
    pub pool: Arc<PgPool>,
}

#[async_trait]
impl ResourceLimitsProvider for ZieeResourceLimitsProvider {
    async fn load_from_db(&self) -> Result<CodeSandboxResourceLimits, AppError> {
        CodeSandboxRepository::new((*self.pool).clone())
            .get_resource_limits()
            .await
    }
}

/// Embedded guest-agent staging. The `include_bytes!` bodies (`embedded.rs` /
/// `wsl2_agent_embedded.rs`) read the SERVER `CARGO_MANIFEST_DIR`, so they stay
/// in this crate; this unit provider forwards to them.
pub struct ZieeGuestAgentProvider;

impl GuestAgentProvider for ZieeGuestAgentProvider {
    fn ensure(&self) -> Result<&'static Extracted, String> {
        crate::modules::code_sandbox::embedded::ensure()
    }

    fn ensure_wsl2(&self) -> Result<&'static PathBuf, String> {
        #[cfg(target_os = "windows")]
        {
            crate::modules::code_sandbox::wsl2_agent_embedded::ensure()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("wsl2 guest agent is only available on Windows".to_string())
        }
    }
}
