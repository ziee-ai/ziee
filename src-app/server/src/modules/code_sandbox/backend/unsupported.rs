//! Fallback backend for any OS that isn't Linux/macOS/Windows. Keeps the crate
//! compiling everywhere; every sandbox operation returns a clear error.

use std::path::Path;

use async_trait::async_trait;

use super::SandboxBackend;
use crate::common::AppError;
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::runtime_mount::{EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::SandboxRunResult;
use crate::modules::code_sandbox::types::{CodeSandboxState, HostCapabilities, SandboxContext};

pub struct UnsupportedBackend;

impl UnsupportedBackend {
    pub fn new() -> Self {
        Self
    }
}

fn unsupported_err() -> AppError {
    AppError::new(
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "SANDBOX_BACKEND_UNSUPPORTED",
        "code sandbox is not supported on this operating system",
    )
}

#[async_trait]
impl SandboxBackend for UnsupportedBackend {
    fn probe_host(&self, _cfg: &CodeSandboxConfig) -> Option<HostCapabilities> {
        tracing::warn!(
            "code_sandbox: OS {} is not supported; sandbox MCP row will NOT be \
             registered.",
            std::env::consts::OS
        );
        None
    }

    async fn ensure_rootfs_ready(
        &self,
        _state: &CodeSandboxState,
        _flavor: &str,
    ) -> Result<EnsureOutcome, AppError> {
        Err(unsupported_err())
    }

    async fn run(
        &self,
        _state: &CodeSandboxState,
        _ctx: &SandboxContext,
        _command: &str,
        _timeout_secs: Option<u64>,
        _flavor: &str,
    ) -> Result<SandboxRunResult, AppError> {
        Err(unsupported_err())
    }

    async fn shutdown(&self) {}

    async fn evict_flavor(&self, _cache_dir: &Path, _flavor: &str) -> EvictOutcome {
        EvictOutcome { bytes_freed: 0, was_cached: false }
    }
}
