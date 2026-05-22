//! Linux backend: bwrap runs directly on the host (today's behavior).
//!
//! Thin delegation to the existing functions — `sandbox::run_in_sandbox`,
//! `runtime_mount::{ensure_rootfs_ready, shutdown, evict_flavor}` — so the
//! audited Linux path is byte-identical; the seam only changes *who calls it*.

use std::path::Path;

use async_trait::async_trait;

use super::SandboxBackend;
use crate::common::AppError;
use crate::modules::code_sandbox::runtime_mount::{self, EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::{self, SandboxRunResult};
use crate::modules::code_sandbox::types::{CodeSandboxState, SandboxContext};

pub struct LinuxBwrapBackend;

impl LinuxBwrapBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SandboxBackend for LinuxBwrapBackend {
    async fn ensure_rootfs_ready(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
    ) -> Result<EnsureOutcome, AppError> {
        runtime_mount::ensure_rootfs_ready(state, flavor).await
    }

    async fn run(
        &self,
        state: &CodeSandboxState,
        ctx: &SandboxContext,
        command: &str,
        timeout_secs: Option<u64>,
        flavor: &str,
    ) -> Result<SandboxRunResult, AppError> {
        sandbox::run_in_sandbox(state, ctx, command, timeout_secs, flavor).await
    }

    async fn shutdown(&self) {
        runtime_mount::shutdown().await
    }

    async fn evict_flavor(&self, cache_dir: &Path, flavor: &str) -> EvictOutcome {
        runtime_mount::evict_flavor(cache_dir, flavor).await
    }
}
