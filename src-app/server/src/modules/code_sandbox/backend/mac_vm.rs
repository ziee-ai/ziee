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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use sandbox_vm_protocol::{encode, Decoder, ExecRequest, Frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use super::SandboxBackend;
use crate::common::AppError;
use crate::modules::code_sandbox::runtime_fetch;
use crate::modules::code_sandbox::runtime_mount::{cache_dir, EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::{
    self, SandboxRunResult, DEFAULT_TIMEOUT_SECS, OUTPUT_CAP_BYTES,
};
use crate::modules::code_sandbox::types::{
    CgroupMode, CodeSandboxState, HardeningCapabilities, PidNsMode, SandboxContext, SeccompMode,
};

// ── Guest contract (must match sandbox-guest-agent constants) ────────────────
const GUEST_VSOCK_PORT: u32 = 1024;
const GUEST_ROOTFS_MOUNT: &str = "/sandbox-rootfs";
const GUEST_WORKSPACE_MOUNT: &str = "/workspace";
const GUEST_BWRAP_PATH: &str = "/usr/bin/bwrap";
const GUEST_AGENT_PATH: &str = "/usr/bin/ziee-sandbox-agent";
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

/// A booted, warm per-flavor VM.
struct VmHandle {
    child: Mutex<tokio::process::Child>,
    socket_path: PathBuf,
    last_used: Mutex<Instant>,
    /// In-flight exec count — the reaper never evicts a VM with a running
    /// command (a long command keeps the VM alive past the idle threshold).
    inflight: AtomicUsize,
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
        let mut vms = VMS.lock().await;
        if let Some(h) = vms.get(flavor) {
            return Ok(h.clone());
        }

        let socket_path = std::env::temp_dir().join(format!("ziee-vm-{flavor}.sock"));
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
        let cfg_path = std::env::temp_dir().join(format!("ziee-vm-{flavor}.json"));
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
        });
        vms.insert(flavor.to_string(), handle.clone());
        ensure_reaper();
        Ok(handle)
    }
}

/// Map a host workspace path to its guest equivalent under the virtio-fs share.
fn guest_workspace_path(state: &CodeSandboxState, host_ws: &Path) -> PathBuf {
    let rel = host_ws.strip_prefix(&state.workspace_root).unwrap_or(host_ws);
    Path::new(GUEST_WORKSPACE_MOUNT).join(rel)
}

/// Send the bwrap argv to the guest agent over the bridged unix socket and
/// collect the streamed output into a `SandboxRunResult`.
async fn run_via_socket(
    socket_path: &Path,
    req: ExecRequest,
    timeout_secs: u64,
) -> Result<SandboxRunResult, AppError> {
    let started = Instant::now();
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| AppError::internal_error(format!("connect VM socket: {e}")))?;
    stream
        .write_all(&encode(&Frame::Exec(req)))
        .await
        .map_err(|e| AppError::internal_error(format!("send exec to VM: {e}")))?;

    let mut decoder = Decoder::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;
    let mut exit_code = -1;
    let mut timed_out = false;

    // Host-side hung-guest guard (gap #6): the agent enforces the per-exec
    // timeout in-guest and should always send Exit, but if the agent itself
    // wedges, bound the host wait at the exec budget + grace.
    let read_budget = Duration::from_secs(timeout_secs + 30);
    loop {
        let n = match tokio::time::timeout(read_budget, stream.read(&mut buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(AppError::internal_error(format!("read VM stream: {e}"))),
            Err(_) => {
                timed_out = true;
                break;
            }
        };
        if n == 0 {
            break; // socket closed
        }
        decoder.feed(&buf[..n]);
        let mut done = false;
        loop {
            match decoder.next_frame() {
                Ok(Some(Frame::Stdout(b))) => append_capped(&mut stdout, &b, &mut stdout_truncated),
                Ok(Some(Frame::Stderr(b))) => append_capped(&mut stderr, &b, &mut stderr_truncated),
                Ok(Some(Frame::Exit(s))) => {
                    exit_code = s.code;
                    timed_out = s.timed_out;
                    done = true;
                    break;
                }
                Ok(Some(Frame::Exec(_))) => {} // not expected from the guest
                Ok(None) => break,
                Err(e) => return Err(AppError::internal_error(format!("VM protocol error: {e}"))),
            }
        }
        if done {
            break;
        }
    }

    Ok(SandboxRunResult {
        exit_code,
        stdout: lossy(stdout, stdout_truncated),
        stderr: lossy(stderr, stderr_truncated),
        stdout_truncated,
        stderr_truncated,
        duration_ms: started.elapsed().as_millis() as u64,
        timed_out,
    })
}

fn append_capped(buf: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    if *truncated {
        return;
    }
    if buf.len() + chunk.len() > OUTPUT_CAP_BYTES {
        let remain = OUTPUT_CAP_BYTES - buf.len();
        buf.extend_from_slice(&chunk[..remain]);
        *truncated = true;
    } else {
        buf.extend_from_slice(chunk);
    }
}

fn lossy(buf: Vec<u8>, truncated: bool) -> String {
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        s.push_str(&format!("\n[output truncated at {OUTPUT_CAP_BYTES} bytes]\n"));
    }
    s
}

#[async_trait]
impl SandboxBackend for MacVmBackend {
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
            cgroup: CgroupMode::None, // MAC-TODO: in-guest cgroup v2 delegation
            seccomp: SeccompMode::NotLinked, // MAC-TODO: compile seccomp for guest
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

        let vm = self.ensure_vm(state, flavor, &disk).await?;
        // Mark in-flight so the idle reaper won't evict this VM mid-command,
        // even for a long-running command that outlasts the idle threshold.
        vm.inflight.fetch_add(1, Ordering::SeqCst);
        *vm.last_used.lock().await = Instant::now();

        // Build the bwrap argv with GUEST paths so the agent can exec it
        // verbatim. The hardening flags are identical to the Linux backend.
        let guest_caps = HardeningCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            pid_namespace: PidNsMode::Strict,
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        };
        // Re-point the workspace at its guest virtio-fs location.
        // MAC-TODO: conversation attachments — build_bwrap_argv derives their
        // bind source from state.workspace_root (host path); for the guest
        // those must map under GUEST_WORKSPACE_MOUNT too. Handle once
        // attachments are exercised on macOS.
        let guest_ctx = SandboxContext {
            conversation_id: ctx.conversation_id,
            user_id: ctx.user_id,
            workspace: guest_workspace_path(state, &ctx.workspace),
            files: ctx.files.clone(),
        };
        let argv = sandbox::build_bwrap_argv(
            &guest_caps,
            state,
            &guest_ctx,
            Path::new(GUEST_ROOTFS_MOUNT),
            command,
            Path::new(GUEST_PASSWD),
            Path::new(GUEST_GROUP),
            None,
        );

        let secs = timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
        let req = ExecRequest {
            request_id: rand_request_id(),
            bwrap_path: GUEST_BWRAP_PATH.to_string(),
            argv,
            timeout_ms: secs * 1000,
        };
        let result = run_via_socket(&vm.socket_path, req, secs).await;
        vm.inflight.fetch_sub(1, Ordering::SeqCst);
        *vm.last_used.lock().await = Instant::now();
        result
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

/// Cheap non-crypto request id for log correlation.
fn rand_request_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
}
