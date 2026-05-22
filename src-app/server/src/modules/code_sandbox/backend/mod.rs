//! Cross-platform sandbox backend seam (Plan 1).
//!
//! Everything in `code_sandbox` runs commands, mounts the rootfs, evicts a
//! flavor, and shuts down through [`active()`] — without knowing *where* bwrap
//! actually executes:
//!
//! | Backend            | bwrap runs            | gate                       |
//! |--------------------|-----------------------|----------------------------|
//! | `LinuxBwrapBackend`| host, directly        | `cfg(target_os = "linux")` |
//! | `MacVmBackend`     | inside a libkrun VM   | `cfg(target_os = "macos")` |
//! | `Wsl2Backend`      | inside a WSL2 distro  | `cfg(target_os = "windows")`|
//!
//! **Key invariant:** `sandbox::build_bwrap_argv` and the rootfs *fetch*
//! coordination (`runtime_fetch`) stay shared and OS-independent — every
//! backend produces the identical bwrap argv; they differ only in where it is
//! spawned and how the rootfs is mounted. This module is the seam that lets the
//! whole crate compile on all three OSes (the Linux-only primitives —
//! `pre_exec`, `libc::prctl`/`kill`, the seccomp pipe — live behind the same
//! `cfg(target_os = "linux")` gate as `LinuxBwrapBackend`).

use std::path::Path;

use async_trait::async_trait;
use once_cell::sync::Lazy;

use crate::common::AppError;
use crate::modules::code_sandbox::runtime_mount::{EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::SandboxRunResult;
use crate::modules::code_sandbox::types::{CodeSandboxState, SandboxContext};

#[cfg(target_os = "linux")]
mod linux_bwrap;
// Shared host-side client for the in-guest agent (vsock/unix on macOS, TCP on
// Windows) — used by both VM backends.
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod vm_client;
#[cfg(target_os = "macos")]
mod mac_vm;
#[cfg(target_os = "windows")]
mod wsl2;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;

/// The execution + lifecycle operations that differ by host OS. Selected once
/// at process start via `cfg` and reached through [`active()`].
#[async_trait]
pub trait SandboxBackend: Send + Sync {
    /// Make the requested flavor's rootfs available (fetch if missing, then
    /// mount in whatever way this backend mounts). The fetch half is shared
    /// (`runtime_fetch`); only the mount differs per backend.
    async fn ensure_rootfs_ready(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
    ) -> Result<EnsureOutcome, AppError>;

    /// Run `command` in an isolated sandbox against `flavor`'s rootfs. The argv
    /// (`sandbox::build_bwrap_argv`) is identical across backends; this method
    /// owns *where* it runs (host / microVM / WSL2) and capture/timeout.
    async fn run(
        &self,
        state: &CodeSandboxState,
        ctx: &SandboxContext,
        command: &str,
        timeout_secs: Option<u64>,
        flavor: &str,
    ) -> Result<SandboxRunResult, AppError>;

    /// Tear down anything this backend owns (FUSE daemons / VMs / distros).
    async fn shutdown(&self);

    /// Evict a flavor from the local cache (unmount + delete). Idempotent.
    async fn evict_flavor(&self, cache_dir: &Path, flavor: &str) -> EvictOutcome;
}

#[cfg(target_os = "linux")]
static ACTIVE: Lazy<linux_bwrap::LinuxBwrapBackend> =
    Lazy::new(linux_bwrap::LinuxBwrapBackend::new);
#[cfg(target_os = "macos")]
static ACTIVE: Lazy<mac_vm::MacVmBackend> = Lazy::new(mac_vm::MacVmBackend::new);
#[cfg(target_os = "windows")]
static ACTIVE: Lazy<wsl2::Wsl2Backend> = Lazy::new(wsl2::Wsl2Backend::new);
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
static ACTIVE: Lazy<unsupported::UnsupportedBackend> =
    Lazy::new(unsupported::UnsupportedBackend::new);

/// The backend for this host OS. Resolved once via `cfg`.
pub fn active() -> &'static dyn SandboxBackend {
    &*ACTIVE
}
