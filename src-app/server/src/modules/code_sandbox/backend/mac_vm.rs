//! macOS backend: bwrap runs inside a libkrun Linux microVM (Plan 1 §2).
//!
//! Scaffold only — the seam compiles and selects this backend on macOS, but the
//! libkrun boot + vsock agent + in-guest squashfs mount land in the §2 slice.
//! Until then every operation returns a clear "not yet implemented" error so
//! the failure mode is explicit rather than a silent panic.

use std::path::Path;

use async_trait::async_trait;

use super::SandboxBackend;
use crate::common::AppError;
use crate::modules::code_sandbox::runtime_mount::{EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::SandboxRunResult;
use crate::modules::code_sandbox::types::{CodeSandboxState, SandboxContext};

pub struct MacVmBackend;

impl MacVmBackend {
    pub fn new() -> Self {
        Self
    }
}

fn unimplemented_err() -> AppError {
    AppError::new(
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "SANDBOX_BACKEND_UNIMPLEMENTED",
        "code sandbox on macOS (libkrun microVM) is not yet implemented (Plan 1 §2)",
    )
}

#[async_trait]
impl SandboxBackend for MacVmBackend {
    async fn ensure_rootfs_ready(
        &self,
        _state: &CodeSandboxState,
        _flavor: &str,
    ) -> Result<EnsureOutcome, AppError> {
        Err(unimplemented_err())
    }

    async fn run(
        &self,
        _state: &CodeSandboxState,
        _ctx: &SandboxContext,
        _command: &str,
        _timeout_secs: Option<u64>,
        _flavor: &str,
    ) -> Result<SandboxRunResult, AppError> {
        Err(unimplemented_err())
    }

    async fn shutdown(&self) {}

    async fn evict_flavor(&self, _cache_dir: &Path, _flavor: &str) -> EvictOutcome {
        EvictOutcome { bytes_freed: 0, was_cached: false }
    }
}
