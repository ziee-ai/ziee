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
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::runtime_mount::{EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::SandboxRunResult;
use crate::modules::code_sandbox::types::{CodeSandboxState, HostCapabilities, SandboxContext};

#[cfg(target_os = "linux")]
mod linux_bwrap;
// Shared host-side client for the in-guest agent (vsock/unix on macOS,
// AF_HYPERV vsock on Windows) — used by both VM backends.
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod vm_client;
// Long-lived multiplexed session over a single agent connection —
// the host side of the MCP-in-sandbox protocol extension. Pure
// Rust+tokio, intentionally not platform-gated: the type needs to
// exist on Linux too so `SandboxBackend::open_long_lived_session`
// can have a uniform return signature (Linux returns Ok(None)).
pub(crate) mod vm_long_lived;
#[cfg(target_os = "macos")]
mod mac_vm;
// AF_HYPERV (Hyper-V vsock) FFI + WSL utility-VM resolution (HIGH-1 fix).
#[cfg(target_os = "windows")]
mod hvsocket;
#[cfg(target_os = "windows")]
mod wsl2;
// LocalSystem helper service: brokers the privileged WSL ops (VmId
// resolution + vsock-port registration) so the unprivileged server never
// needs Hyper-V Admin or runtime UAC. Mirrors Docker Desktop's model.
#[cfg(target_os = "windows")]
pub(crate) mod helper_service;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;

/// The execution + lifecycle operations that differ by host OS. Selected once
/// at process start via `cfg` and reached through [`active()`].
#[async_trait]
pub trait SandboxBackend: Send + Sync {
    /// One-time boot probe. Cheap, host-only, sync (sub-10 ms). `None` means
    /// "this host cannot run the sandbox" — `init()` then logs the reason and
    /// skips MCP-row registration entirely (the rest of the server boots
    /// fine). Per-backend prerequisites:
    ///   - Linux: `bwrap` on PATH, cgroup probe, seccomp-filter compile.
    ///   - macOS: launcher reachable + arch is aarch64 (libkrun is Apple-Silicon).
    ///   - Windows: `wsl.exe` present + reports WSL v2 (v1-only refuses).
    ///   - Unsupported OS: always `None`.
    ///
    /// The returned `HostCapabilities` is consumed by the Linux path
    /// (`runtime_mount` re-uses `bwrap_path` for the lazy PID-ns probe); on the
    /// VM backends it is an opaque placeholder — the real *guest* caps are
    /// rebuilt per-call in `ensure_rootfs_ready` / `run`.
    fn probe_host(&self, cfg: &CodeSandboxConfig) -> Option<HostCapabilities>;

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

    /// **Test-only seam.** Execute an arbitrary `bwrap` argv against the
    /// given rootfs and return the raw stdout/stderr/exit. Tier-4 tests
    /// use this to verify the hardening primitives the argv builder
    /// emits (seccomp blocks foo, --ro-bind makes /usr read-only, etc.)
    /// without naming `bwrap` themselves.
    ///
    /// Per-backend dispatch:
    ///   - Linux: `Command::new("bwrap").args(argv)` directly on the host.
    ///   - macOS: ensure a libkrun VM is up for `rootfs_squashfs`,
    ///     send `Frame::Exec { bwrap_path: "/usr/bin/bwrap", argv, .. }`
    ///     to the in-guest agent, collect frames.
    ///   - Windows: equivalent vsock dispatch via the WSL2 agent.
    ///   - Unsupported: error out.
    ///
    /// `rootfs_squashfs` is the path to a `.squashfs` file (Mac/Windows
    /// pass it to libkrun/WSL2 as a virtio-blk disk; Linux ignores it
    /// — host bwrap reads from whatever path the argv references).
    async fn exec_raw_argv(
        &self,
        argv: Vec<String>,
        rootfs_squashfs: &Path,
        timeout: std::time::Duration,
    ) -> Result<RawExecResult, AppError>;

    /// Open a long-lived multiplexed session for a sandboxed MCP
    /// stdio server using `flavor`. The MCP client holds the returned
    /// `LongLivedSession` for the server's lifetime; dropping it tears
    /// down the underlying connection (and, on VM backends, releases
    /// the per-flavor inflight guard that keeps the VM warm).
    ///
    /// Backends:
    ///   - **Linux** — `Ok(None)` (default). The Linux MCP path spawns
    ///     bwrap directly via `Command::new(bwrap_path).args(argv)`;
    ///     no agent is involved.
    ///   - **macOS** — ensure the flavor's libkrun VM is warm, dial a
    ///     fresh `UnixStream` against its vsock bridge socket, wrap in
    ///     `open_long_lived_with_guard`. Returns `Ok(Some(session))`.
    ///   - **Windows (WSL2)** — equivalent dial against the AF_HYPERV
    ///     vsock; same wrap.
    ///   - **Unsupported** — `Ok(None)` (the caller falls back to a
    ///     friendly "sandbox not available" error).
    ///
    /// Default implementation returns `Ok(None)` so backends that
    /// can't or shouldn't expose a session don't need to override.
    async fn open_long_lived_session(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
    ) -> Result<Option<vm_long_lived::LongLivedSession>, AppError> {
        let _ = (state, flavor);
        Ok(None)
    }

    /// Ensure the per-server MCP workspace exists where the long-lived bwrap
    /// argv binds it (`/workspace/mcp/<server_id>` → `/home/sandboxuser`).
    ///
    /// Default no-op: on macOS the host workspace is virtio-fs-shared at
    /// `/workspace`, and on the Linux host path bwrap binds the host dir
    /// directly — so the bind source already exists. The WSL2 backend has no
    /// virtio-fs, so it overrides this to create + rsync the per-server
    /// workspace into the distro before the spawn (mirroring the one-shot
    /// `run` path's `sync_workspace_in`). Without it, bwrap fails with
    /// "Can't find source path /workspace/mcp/<id>".
    async fn prepare_mcp_vm_workspace(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
        server_id: uuid::Uuid,
    ) -> Result<(), AppError> {
        let _ = (state, flavor, server_id);
        Ok(())
    }
}

/// Captured output of a raw bwrap execution. Used by tier-4 hardening
/// tests via the per-backend `exec_raw_argv` seam.
#[derive(Debug, Clone)]
pub struct RawExecResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// True if killed by the wall-clock timeout (`timeout` arg expired).
    pub timed_out: bool,
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
