//! Linux backend: bwrap runs directly on the host (today's behavior).
//!
//! Thin delegation to the existing functions — `sandbox::run_in_sandbox`,
//! `runtime_mount::{ensure_rootfs_ready, shutdown, evict_flavor}` — so the
//! audited Linux path is byte-identical; the seam only changes *who calls it*.

use std::path::Path;

use async_trait::async_trait;

use super::SandboxBackend;
use crate::common::AppError;
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::probes;
use crate::modules::code_sandbox::runtime_mount::{self, EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::{self, SandboxRunResult};
use crate::modules::code_sandbox::types::{CodeSandboxState, HostCapabilities, SandboxContext};

pub struct LinuxBwrapBackend;

impl LinuxBwrapBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SandboxBackend for LinuxBwrapBackend {
    fn probe_host(&self, cfg: &CodeSandboxConfig) -> Option<HostCapabilities> {
        // Today's behavior: bwrap on PATH + cgroup probe + seccomp-filter compile.
        probes::probe_host_only(cfg)
    }

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

    async fn run_with_mounts(
        &self,
        state: &CodeSandboxState,
        ctx: &SandboxContext,
        command: &str,
        timeout_secs: Option<u64>,
        flavor: &str,
        extra_mounts: &[crate::modules::code_sandbox::workflow_staging::StagedMount],
        progress_tx: Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
    ) -> Result<SandboxRunResult, AppError> {
        // bwrap runs directly on the host, so the host owns the progress FIFO
        // (created + read inside `run_in_sandbox_with_mounts`). Thread the sink
        // straight through.
        sandbox::run_in_sandbox_with_mounts(
            state,
            ctx,
            command,
            timeout_secs,
            flavor,
            extra_mounts,
            progress_tx,
        )
        .await
    }

    fn supports_extra_mounts(&self) -> bool {
        // bwrap binds host paths directly on the host — host folders and
        // workflow staged dirs both work.
        true
    }

    async fn shutdown(&self) {
        runtime_mount::shutdown().await
    }

    async fn evict_flavor(&self, cache_dir: &Path, flavor: &str) -> EvictOutcome {
        runtime_mount::evict_flavor(cache_dir, flavor).await
    }

    async fn exec_raw_argv(
        &self,
        argv: Vec<String>,
        _rootfs_squashfs: &Path,
        timeout: std::time::Duration,
    ) -> Result<super::RawExecResult, AppError> {
        // Linux backend: bwrap on the host directly. `rootfs_squashfs` is
        // unused — Linux tests bind-mount the already-mounted rootfs into
        // bwrap via the argv (`--ro-bind /var/lib/ziee/sandbox-rootfs/current
        // /sandbox-rootfs`). The mount is set up by `runtime_mount` /
        // `just sandbox-mount` before the test runs.
        let output = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("bwrap").args(&argv).output(),
        )
        .await;

        match output {
            Ok(Ok(out)) => Ok(super::RawExecResult {
                exit_code: out.status.code().unwrap_or(-1),
                stdout: out.stdout,
                stderr: out.stderr,
                timed_out: false,
            }),
            Ok(Err(e)) => Err(AppError::internal_error(format!("bwrap spawn failed: {e}"))),
            Err(_) => Ok(super::RawExecResult {
                exit_code: -1,
                stdout: Vec::new(),
                stderr: format!("bwrap timed out after {timeout:?}").into_bytes(),
                timed_out: true,
            }),
        }
    }
}
