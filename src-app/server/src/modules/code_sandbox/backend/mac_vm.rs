//! macOS backend: bwrap runs inside a per-flavor libkrun microVM (Plan 1 §2).
//!
//! Architecture (see also `sandbox-vm-launcher` + `sandbox-guest-agent`):
//!   - One warm **VM per flavor**, booted lazily on first use by spawning the
//!     `ziee-sandbox-vm-launcher` child process (NOT an in-process fork —
//!     `krun_start_enter` `exit()`s and post-fork only async-signal-safe calls
//!     are legal).
//!   - The launcher boots libkrun with: the guest root (agent + bwrap), the
//!     flavor squashfs as a virtio-blk disk, the workspace root via virtio-fs,
//!     and a vsock port bridged to a host unix socket.
//!   - The guest `ziee-sandbox-agent` listens on that vsock port. Per
//!     `execute_command`, this backend connects to the unix socket and sends
//!     the **bwrap argv built by `build_bwrap_argv`** (guest paths) — so the
//!     hardening flags are identical to the Linux backend.
//!
//! ⚠️ **Validation status:** this file cannot be compiled on Linux (no macOS
//! toolchain / libkrun). It is grounded in the real libkrun API + the protocol
//! crate but must be compiled + validated on macOS. Points flagged `MAC-TODO`
//! need first-run attention. See `src-app/sandbox-rootfs/MACOS-RUNBOOK.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use sandbox_vm_protocol::{CgroupLimits, ExecRequest};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, Semaphore};

use super::SandboxBackend;
use crate::common::AppError;
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::runtime_fetch;
use crate::modules::code_sandbox::runtime_mount::{cache_dir, EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::{self, SandboxRunResult, DEFAULT_TIMEOUT_SECS};
use crate::modules::code_sandbox::types::{
    CgroupMode, CodeSandboxState, HardeningCapabilities, HostCapabilities, PidNsMode,
    SandboxContext, SeccompMode,
};

// ── Guest contract (must match sandbox-guest-agent constants) ────────────────
const GUEST_VSOCK_PORT: u32 = 1024;
const GUEST_ROOTFS_MOUNT: &str = "/sandbox-rootfs";
const GUEST_WORKSPACE_MOUNT: &str = "/workspace";
const GUEST_BWRAP_PATH: &str = "/usr/bin/bwrap";
const GUEST_AGENT_PATH: &str = "/usr/bin/ziee-sandbox-agent";
// Fixed fd the agent dup2's the seccomp BPF pipe to in the bwrap child; the
// argv built here references it via `--seccomp <fd>`. Out of the stdio range.
const GUEST_SECCOMP_FD: i32 = 10;
// Guest synthetic identity files — baked into the guest root image (MAC-TODO:
// ensure the guest root ships these; build_bwrap_argv binds them over
// /etc/passwd + /etc/group).
const GUEST_PASSWD: &str = "/etc/ziee-sandbox-passwd";
const GUEST_GROUP: &str = "/etc/ziee-sandbox-group";

// Default VM sizing. MAC-TODO: wire to the §6 runtime-configurable limits.
const VM_VCPUS: u8 = 2;
const VM_RAM_MIB: u32 = 2048;
// Evict a flavor's VM after this long idle with nothing in flight (gap #6 —
// VMs hold RAM). MAC-TODO: wire to config `vm_idle_evict_secs` (0 = never).
const VM_IDLE_EVICT_SECS: u64 = 900;
// Cap concurrent execs per VM so N parallel commands (each cgroup-capped at
// ~512 MiB) can't sum past the VM's RAM ceiling and trigger a guest OOM
// (Ga). ~VM_RAM_MIB / 512 with headroom. MAC-TODO: derive from §6 config.
const MAX_CONCURRENT_EXECS_PER_VM: usize = 3;

/// Monotonic request id (B4) — avoids the cgroup-path collisions a timestamp
/// id risked under concurrency.
static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A booted, warm per-flavor VM.
struct VmHandle {
    child: Mutex<tokio::process::Child>,
    socket_path: PathBuf,
    last_used: Mutex<Instant>,
    /// In-flight exec count — the reaper never evicts a VM with a running
    /// command (a long command keeps the VM alive past the idle threshold).
    inflight: AtomicUsize,
    /// Bounds concurrent execs in this VM (Ga) so they can't OOM the guest.
    sem: Semaphore,
}

/// Decrements `inflight` on drop — so a cancelled `run()` future (aborted chat
/// turn) doesn't leak the count and wedge the reaper (B2).
struct InflightGuard(Arc<VmHandle>);
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Background reaper: started once; evicts idle, not-in-use VMs.
static REAPER_STARTED: AtomicBool = AtomicBool::new(false);

fn ensure_reaper() {
    if REAPER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if VM_IDLE_EVICT_SECS == 0 {
                continue;
            }
            let mut vms = VMS.lock().await;
            let mut evict = Vec::new();
            for (flavor, h) in vms.iter() {
                if h.inflight.load(Ordering::SeqCst) == 0
                    && h.last_used.lock().await.elapsed().as_secs() >= VM_IDLE_EVICT_SECS
                {
                    evict.push(flavor.clone());
                }
            }
            for flavor in evict {
                if let Some(h) = vms.remove(&flavor) {
                    let mut child = h.child.lock().await;
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    let _ = std::fs::remove_file(&h.socket_path);
                    tracing::info!(flavor, "code_sandbox: macOS VM evicted (idle)");
                }
            }
        }
    });
}

/// Per-flavor warm VMs. Boot is serialized by holding this lock across the
/// (rare) launch, which doubles as single-flight — concurrent first-calls for a
/// flavor produce one VM. MAC-TODO: if cross-flavor boot contention matters,
/// switch to a per-flavor OnceCell like `runtime_mount::READY`.
static VMS: Lazy<Mutex<HashMap<String, Arc<VmHandle>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Per-flavor boot serialization (B3) — held only during a boot, NOT during
/// warm reuse, so booting flavor A doesn't block running an already-warm flavor
/// B (the global VMS lock is released across the ≤30 s boot).
static BOOT_LOCKS: Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

async fn boot_lock_for(flavor: &str) -> Arc<Mutex<()>> {
    BOOT_LOCKS
        .lock()
        .await
        .entry(flavor.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Per-server-instance private runtime dir (mode 0700) for VM control sockets +
/// launch configs. Replaces predictable, world-traversable /tmp paths (S1/S2):
/// only the server's uid can reach the (unauthenticated) control socket, and an
/// attacker can't pre-create a symlink at a guessable path.
fn runtime_dir() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("ziee-sandbox-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(dir)
}

pub struct MacVmBackend;

impl MacVmBackend {
    pub fn new() -> Self {
        Self
    }

    /// Resolve the bundled launcher binary + guest root. Overridable via env
    /// for dev; defaults assume the app-bundle layout (Contents/Resources).
    fn launcher_path() -> PathBuf {
        if let Ok(p) = std::env::var("ZIEE_SANDBOX_VM_LAUNCHER") {
            return PathBuf::from(p);
        }
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
            .map(|dir| dir.join("ziee-sandbox-vm-launcher"))
            .unwrap_or_else(|| PathBuf::from("ziee-sandbox-vm-launcher"))
    }

    fn guest_root_path() -> PathBuf {
        std::env::var("ZIEE_SANDBOX_GUEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/opt/ziee/sandbox-guest-root"))
    }

    /// Get the warm VM for `flavor`, booting one (single-flight) if needed.
    async fn ensure_vm(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
        sandbox_disk: &Path,
    ) -> Result<Arc<VmHandle>, AppError> {
        // Fast path: warm VM (don't hold the lock across a boot).
        if let Some(h) = VMS.lock().await.get(flavor) {
            return Ok(h.clone());
        }
        // Serialize boot for THIS flavor only (B3). The global VMS lock is NOT
        // held across the ≤30 s boot, so other flavors stay responsive.
        let boot_lock = boot_lock_for(flavor).await;
        let _boot = boot_lock.lock().await;
        // Re-check: another caller may have booted this flavor while we waited.
        if let Some(h) = VMS.lock().await.get(flavor) {
            return Ok(h.clone());
        }

        let dir = runtime_dir().map_err(|e| AppError::internal_error(format!("runtime dir: {e}")))?;
        let socket_path = dir.join(format!("vm-{flavor}.sock"));
        let _ = std::fs::remove_file(&socket_path);

        let cfg = serde_json::json!({
            "num_vcpus": VM_VCPUS,
            "ram_mib": VM_RAM_MIB,
            "root_path": Self::guest_root_path().to_string_lossy(),
            "sandbox_disk_path": sandbox_disk.to_string_lossy(),
            "workspace_host_path": state.workspace_root.to_string_lossy(),
            "vsock_socket_path": socket_path.to_string_lossy(),
            "vsock_port": GUEST_VSOCK_PORT,
            "agent_exec_path": GUEST_AGENT_PATH,
        });
        let cfg_path = dir.join(format!("vm-{flavor}.json"));
        std::fs::write(&cfg_path, serde_json::to_vec(&cfg).unwrap()).map_err(|e| {
            AppError::internal_error(format!("write VM launch config: {e}"))
        })?;

        // Gap #4: clear the env so the VMM process does not inherit the
        // server's secrets (DATABASE_URL/JWT/API keys). The launcher needs no
        // env — its config is the JSON file arg and libkrun is found via rpath.
        let child = tokio::process::Command::new(Self::launcher_path())
            .arg(&cfg_path)
            .env_clear()
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AppError::internal_error(format!("spawn VM launcher: {e}")))?;

        // Wait for the bridge socket to appear (VM booted + agent listening).
        let deadline = Instant::now() + Duration::from_secs(30);
        while !socket_path.exists() {
            if Instant::now() > deadline {
                return Err(AppError::internal_error(
                    "VM launcher: vsock socket did not appear within 30s",
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let handle = Arc::new(VmHandle {
            child: Mutex::new(child),
            socket_path,
            last_used: Mutex::new(Instant::now()),
            inflight: AtomicUsize::new(0),
            sem: Semaphore::new(MAX_CONCURRENT_EXECS_PER_VM),
        });
        VMS.lock().await.insert(flavor.to_string(), handle.clone());
        ensure_reaper();
        Ok(handle)
    }
}

/// Map a host workspace path to its guest equivalent under the virtio-fs share.
fn guest_workspace_path(state: &CodeSandboxState, host_ws: &Path) -> PathBuf {
    let rel = host_ws.strip_prefix(&state.workspace_root).unwrap_or(host_ws);
    Path::new(GUEST_WORKSPACE_MOUNT).join(rel)
}

#[async_trait]
impl SandboxBackend for MacVmBackend {
    fn probe_host(&self, _cfg: &CodeSandboxConfig) -> Option<HostCapabilities> {
        // libkrun (via Hypervisor.framework) ships only on Apple Silicon, so
        // gate boot on aarch64 + a reachable launcher binary. The launcher
        // itself dlopens libkrun + checks Hypervisor.framework at start; here
        // we just keep init cheap (sub-10 ms, no dlopen, no fs scans).
        if std::env::consts::ARCH != "aarch64" {
            tracing::error!(
                "code_sandbox: macOS backend requires Apple Silicon (aarch64); \
                 host is {}. Sandbox MCP row will NOT be registered.",
                std::env::consts::ARCH
            );
            return None;
        }
        let launcher = Self::launcher_path();
        if !launcher.exists() {
            tracing::error!(
                "code_sandbox: macOS VM launcher not found at {} \
                 (set ZIEE_SANDBOX_VM_LAUNCHER for dev). Sandbox MCP row will \
                 NOT be registered.",
                launcher.display()
            );
            return None;
        }
        // Placeholder caps: the VM run path rebuilds real *guest* caps in
        // `ensure_rootfs_ready` + `run`; the only downstream consumer of this
        // value on the Linux backend (`runtime_mount`'s PID-ns probe) isn't on
        // the macOS code path.
        Some(HostCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        })
    }

    async fn ensure_rootfs_ready(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
    ) -> Result<EnsureOutcome, AppError> {
        // Shared fetch coordination (OS-independent). Mounting happens inside
        // the guest, so there is no host mount dir — the guest mounts the
        // squashfs at GUEST_ROOTFS_MOUNT.
        let cache = cache_dir(state);
        let outcome = runtime_fetch::ensure_fetched(&cache, flavor, |_| {})
            .await
            .map_err(|e| AppError::internal_error(format!("rootfs fetch failed: {e}")))?;

        let guest_caps = HardeningCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            pid_namespace: PidNsMode::Strict,
            // caps.cgroup drives the *host* CgroupScope, unused in the VM — the
            // guest agent applies cgroup v2 from ExecRequest.cgroup. See run().
            cgroup: CgroupMode::None,
            // The caps.seccomp field is unused for the --seccomp flag (that's
            // the build_bwrap_argv seccomp_fd arg). The guest agent builds +
            // applies the shared seccomp filter itself; see run().
            seccomp: SeccompMode::NotLinked,
        };
        Ok(EnsureOutcome {
            caps: Arc::new(guest_caps),
            mount_dir: PathBuf::from(GUEST_ROOTFS_MOUNT),
            fetch_info: Some(outcome),
        })
    }

    async fn run(
        &self,
        state: &CodeSandboxState,
        ctx: &SandboxContext,
        command: &str,
        timeout_secs: Option<u64>,
        flavor: &str,
    ) -> Result<SandboxRunResult, AppError> {
        // Locate (fetch if needed) the flavor squashfs; idempotent on cache hit.
        let cache = cache_dir(state);
        let disk = runtime_fetch::ensure_fetched(&cache, flavor, |_| {})
            .await
            .map_err(|e| AppError::internal_error(format!("rootfs fetch failed: {e}")))?
            .installed_path;

        // Build the bwrap argv with GUEST paths so the agent can exec it
        // verbatim. The hardening flags are identical to the Linux backend.
        let guest_caps = HardeningCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            pid_namespace: PidNsMode::Strict,
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        };
        let guest_ctx = SandboxContext {
            conversation_id: ctx.conversation_id,
            user_id: ctx.user_id,
            workspace: guest_workspace_path(state, &ctx.workspace),
            files: ctx.files.clone(),
        };
        let secs = timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
        // The argv references the guest seccomp fd; the agent builds the same
        // shared-policy BPF and pipes it to that fd. Passing GUEST_WORKSPACE_MOUNT
        // as the attachment root (Gb) makes attachment binds resolve to guest
        // paths under /workspace, not the host workspace_root.
        let req = ExecRequest {
            request_id: REQ_COUNTER.fetch_add(1, Ordering::Relaxed),
            bwrap_path: GUEST_BWRAP_PATH.to_string(),
            argv: sandbox::build_bwrap_argv(
                &guest_caps,
                Path::new(GUEST_WORKSPACE_MOUNT),
                &guest_ctx,
                Path::new(GUEST_ROOTFS_MOUNT),
                command,
                Path::new(GUEST_PASSWD),
                Path::new(GUEST_GROUP),
                Some(GUEST_SECCOMP_FD),
            ),
            timeout_ms: secs * 1000,
            seccomp_fd: Some(GUEST_SECCOMP_FD),
            // In-guest cgroup v2 limits (the agent applies them; prlimit in the
            // argv is the backstop). MAC-TODO: source from §6 config when it lands.
            cgroup: Some(CgroupLimits::default_policy()),
        };

        // Up to 2 attempts: a dead/unreachable VM (connect fails — the command
        // never ran, so retry is safe) is evicted + re-booted once (B1). A
        // failure AFTER connect is NOT retried (the command may have started).
        let mut attempt = 0;
        loop {
            attempt += 1;
            let vm = self.ensure_vm(state, flavor, &disk).await?;
            // Bound concurrency per VM (Ga) and mark in-flight so the reaper
            // won't evict mid-command (with a Drop guard so a cancelled future
            // can't leak the count — B2).
            let _permit = vm.sem.acquire().await.expect("VM semaphore never closed");
            vm.inflight.fetch_add(1, Ordering::SeqCst);
            let _guard = InflightGuard(vm.clone());
            *vm.last_used.lock().await = Instant::now();

            match UnixStream::connect(&vm.socket_path).await {
                Ok(stream) => {
                    let result = super::vm_client::run_on_stream(stream, req.clone(), secs).await;
                    *vm.last_used.lock().await = Instant::now();
                    return result;
                }
                Err(e) if attempt < 2 => {
                    tracing::warn!(flavor, "code_sandbox: VM unreachable ({e}); re-booting and retrying");
                    drop(_guard);
                    drop(_permit);
                    evict_dead_vm(flavor, &vm).await;
                    continue;
                }
                Err(e) => {
                    return Err(AppError::internal_error(format!("connect VM socket: {e}")));
                }
            }
        }
    }

    async fn shutdown(&self) {
        let mut vms = VMS.lock().await;
        for (flavor, handle) in vms.drain() {
            let mut child = handle.child.lock().await;
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = std::fs::remove_file(&handle.socket_path);
            tracing::info!(flavor, "code_sandbox: macOS VM stopped on shutdown");
        }
    }

    async fn evict_flavor(&self, cache_dir: &Path, flavor: &str) -> EvictOutcome {
        // Stop the flavor's VM if running.
        if let Some(handle) = VMS.lock().await.remove(flavor) {
            let mut child = handle.child.lock().await;
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = std::fs::remove_file(&handle.socket_path);
        }
        // Delete the cached squashfs for this flavor.
        let suffix = format!("-{flavor}.squashfs");
        let mut bytes_freed = 0;
        let mut was_cached = false;
        if let Ok(rd) = std::fs::read_dir(cache_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(&suffix)) {
                    was_cached = true;
                    if let Ok(m) = std::fs::metadata(&p) {
                        bytes_freed += m.len();
                    }
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        EvictOutcome { bytes_freed, was_cached }
    }
}

/// Remove a dead/unreachable VM from the registry (only if it's still the
/// current handle for the flavor — don't clobber a concurrent fresh boot) and
/// kill its launcher, so the next `ensure_vm` re-boots (B1). Idempotent.
async fn evict_dead_vm(flavor: &str, dead: &Arc<VmHandle>) {
    {
        let mut vms = VMS.lock().await;
        if vms.get(flavor).is_some_and(|h| Arc::ptr_eq(h, dead)) {
            vms.remove(flavor);
        }
    }
    let mut child = dead.child.lock().await;
    let _ = child.start_kill();
    let _ = child.wait().await;
    let _ = std::fs::remove_file(&dead.socket_path);
}
