//! Windows backend: bwrap runs inside a per-flavor WSL2 distro (Plan 1 §3).
//!
//! Architecture (full parity with the macOS backend — same `sandbox-guest-agent`,
//! same control protocol, same `build_bwrap_argv` hardening; only the transport
//! and the "VM" differ):
//!   - The flavor rootfs is fetched as a **`.tar.zst`** (the squashfs can't be
//!     `wsl --import`ed) and registered as a distro: `wsl --import
//!     ziee-sandbox-<flavor>-v<schema> <dir> <tarball> --version 2`. The imported
//!     distro filesystem **is** the flavor rootfs (R/torch/etc.).
//!   - A one-time provision step (run as root in the distro) installs `bwrap`,
//!     drops in the `ziee-sandbox-agent` binary, writes the synthetic
//!     passwd/group, and — **critically** — flips the unprivileged-userns
//!     sysctls bwrap's `--unshare-user` needs (OFF by default in WSL2; further
//!     restricted by AppArmor on Ubuntu 24.04/noble, this rootfs's base).
//!   - The agent is started inside the distro listening on **127.0.0.1:<port>**;
//!     WSL2's localhost-forwarding makes that reachable from Windows. Per
//!     `execute_command` this backend connects a `TcpStream` and sends the
//!     bwrap argv (guest paths) — the agent applies the shared seccomp + cgroup
//!     in-guest, identical to macOS.
//!
//! ⚠️ **Validation status:** this file cannot be compiled or run on Linux (it is
//! `cfg(target_os = "windows")`). It is grounded in the documented WSL2 + agent
//! behavior but must be compiled + validated on Windows 11 + WSL2. Points
//! flagged `WIN-TODO` need first-run attention.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use sandbox_vm_protocol::{CgroupLimits, ExecRequest, PROTOCOL_VERSION};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};

use super::SandboxBackend;
use crate::common::AppError;
use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::runtime_fetch::{self, RootfsFormat};
use crate::modules::code_sandbox::runtime_mount::{cache_dir, EnsureOutcome, EvictOutcome};
use crate::modules::code_sandbox::sandbox::{
    self, SandboxRunResult, DEFAULT_TIMEOUT_SECS, SYNTHETIC_GROUP, SYNTHETIC_PASSWD,
};
use crate::modules::code_sandbox::types::{
    CgroupMode, CodeSandboxState, HardeningCapabilities, HostCapabilities, PidNsMode,
    SandboxContext, SeccompMode,
};
use crate::modules::code_sandbox::SANDBOX_ROOTFS_SCHEMA_VERSION;

// ── Guest contract (paths INSIDE the imported distro) ────────────────────────
// The distro filesystem is the flavor rootfs, so bwrap's rootfs_dir is "/"
// (`build_bwrap_argv` binds `{rootfs}/usr` → `/usr`; `//usr` == `/usr`).
const GUEST_ROOTFS_MOUNT: &str = "/";
const GUEST_BWRAP_PATH: &str = "/usr/bin/bwrap";
const GUEST_AGENT_PATH: &str = "/usr/local/bin/ziee-sandbox-agent";
// Fixed fd the agent dup2's the seccomp BPF pipe to in the bwrap child (matches
// the macOS backend + the agent's GUEST_SECCOMP_FD). Out of the stdio range.
const GUEST_SECCOMP_FD: i32 = 10;
const GUEST_PASSWD: &str = "/etc/ziee-sandbox-passwd";
const GUEST_GROUP: &str = "/etc/ziee-sandbox-group";
// Marker written at the end of a successful provision so re-boots skip it.
const PROVISION_SENTINEL: &str = "/etc/ziee-sandbox-provisioned";

// Evict a flavor's distro after this long idle with nothing in flight.
// WIN-TODO: wire to config `vm_idle_evict_secs` (0 = never) alongside macOS.
const IDLE_EVICT_SECS: u64 = 900;
// Cap concurrent execs per distro so N parallel commands (each cgroup-capped)
// can't sum past the WSL2 VM's RAM. Mirrors the macOS `MAX_CONCURRENT_EXECS_PER_VM`.
const MAX_CONCURRENT_EXECS: usize = 3;

/// Monotonic request id (matches the macOS B4 fix — avoids cgroup-path
/// collisions a timestamp id risked under concurrency).
static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A booted, warm per-flavor distro: the imported WSL2 distro plus the agent
/// process listening on a localhost TCP port.
struct DistroHandle {
    /// The `wsl.exe -d <distro> -- agent …` relay child. Killing it stops the
    /// agent (WIN-TODO: confirm the in-distro agent dies when this relay is
    /// killed; if WSL keeps it alive, fall back to `wsl --terminate <distro>`).
    agent: Mutex<tokio::process::Child>,
    distro: String,
    tcp_port: u16,
    last_used: Mutex<Instant>,
    /// In-flight exec count — the reaper never evicts a distro mid-command.
    inflight: AtomicUsize,
    /// Bounds concurrent execs in this distro so they can't OOM the WSL2 VM.
    sem: Semaphore,
}

/// Decrements `inflight` on drop so a cancelled `run()` future (aborted chat
/// turn) can't leak the count and wedge the reaper (mirrors macOS B2).
struct InflightGuard(Arc<DistroHandle>);
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}

static REAPER_STARTED: AtomicBool = AtomicBool::new(false);

fn ensure_reaper() {
    if REAPER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if IDLE_EVICT_SECS == 0 {
                continue;
            }
            let mut distros = DISTROS.lock().await;
            let mut evict = Vec::new();
            for (flavor, h) in distros.iter() {
                if h.inflight.load(Ordering::SeqCst) == 0
                    && h.last_used.lock().await.elapsed().as_secs() >= IDLE_EVICT_SECS
                {
                    evict.push(flavor.clone());
                }
            }
            for flavor in evict {
                if let Some(h) = distros.remove(&flavor) {
                    stop_agent(&h).await;
                    // Terminate the distro too so its slice of the shared WSL2
                    // VM's RAM is freed (the agent alone may not release it).
                    let _ = run_wsl(&["--terminate", &h.distro]).await;
                    tracing::info!(flavor, distro = %h.distro, "code_sandbox: WSL2 distro evicted (idle)");
                }
            }
        }
    });
}

/// Per-flavor warm distros.
static DISTROS: Lazy<Mutex<HashMap<String, Arc<DistroHandle>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Per-flavor boot serialization (mirrors macOS B3) — held only during a boot,
/// not during warm reuse, so booting flavor A doesn't block running flavor B.
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

pub struct Wsl2Backend;

impl Wsl2Backend {
    pub fn new() -> Self {
        Self
    }

    /// The bundled Linux `ziee-sandbox-agent` binary (Windows-side path),
    /// copied into the distro at provision time. Overridable via env for dev.
    fn agent_host_path() -> PathBuf {
        if let Ok(p) = std::env::var("ZIEE_SANDBOX_AGENT") {
            return PathBuf::from(p);
        }
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
            .map(|dir| dir.join("ziee-sandbox-agent"))
            .unwrap_or_else(|| PathBuf::from("ziee-sandbox-agent"))
    }

    fn distro_name(flavor: &str) -> String {
        format!("ziee-sandbox-{flavor}-v{SANDBOX_ROOTFS_SCHEMA_VERSION}")
    }

    /// Per-distro install dir (where `wsl --import` lays down the ext4 vhdx).
    fn import_dir(state: &CodeSandboxState, flavor: &str) -> PathBuf {
        cache_dir(state).join("wsl").join(Self::distro_name(flavor))
    }

    /// Get the warm distro for `flavor`, importing + provisioning + starting the
    /// agent (single-flight) if needed.
    async fn ensure_distro(
        &self,
        state: &CodeSandboxState,
        flavor: &str,
        tarball: &Path,
    ) -> Result<Arc<DistroHandle>, AppError> {
        // Fast path: warm distro (don't hold the lock across a boot).
        if let Some(h) = DISTROS.lock().await.get(flavor) {
            return Ok(h.clone());
        }
        let boot_lock = boot_lock_for(flavor).await;
        let _boot = boot_lock.lock().await;
        if let Some(h) = DISTROS.lock().await.get(flavor) {
            return Ok(h.clone());
        }

        let distro = Self::distro_name(flavor);

        // 1. Import the distro if it isn't already registered (idempotent).
        if !distro_registered(&distro).await {
            let import_dir = Self::import_dir(state, flavor);
            std::fs::create_dir_all(&import_dir)
                .map_err(|e| AppError::internal_error(format!("create WSL import dir: {e}")))?;
            run_wsl(&[
                "--import",
                &distro,
                &import_dir.to_string_lossy(),
                &tarball.to_string_lossy(),
                "--version",
                "2",
            ])
            .await
            .map_err(|e| AppError::internal_error(format!("wsl --import {distro}: {e}")))?;
        }

        // 2. Provision once (bwrap, agent, identity, userns sysctls).
        self.provision_distro(&distro).await?;

        // 3. Start the agent on a localhost TCP port.
        let port = portpicker::pick_unused_port()
            .ok_or_else(|| AppError::internal_error("no free TCP port for WSL2 agent"))?;
        // The server env does NOT cross into the distro (only WSLENV-listed vars
        // do, and our secrets aren't listed), and bwrap `--clearenv` is the real
        // env defense regardless — so no env_clear dance is needed here.
        let agent = tokio::process::Command::new("wsl.exe")
            .args([
                "-d",
                &distro,
                "-u",
                "root",
                "--cd",
                "/",
                "--",
                GUEST_AGENT_PATH,
                "--listen",
                &format!("tcp:127.0.0.1:{port}"),
            ])
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AppError::internal_error(format!("spawn WSL2 agent: {e}")))?;

        // Wait for the agent to accept connections (localhost-forwarded).
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                break;
            }
            if Instant::now() > deadline {
                return Err(AppError::internal_error(
                    "WSL2 agent did not start listening within 30s",
                ));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        let handle = Arc::new(DistroHandle {
            agent: Mutex::new(agent),
            distro,
            tcp_port: port,
            last_used: Mutex::new(Instant::now()),
            inflight: AtomicUsize::new(0),
            sem: Semaphore::new(MAX_CONCURRENT_EXECS),
        });
        DISTROS.lock().await.insert(flavor.to_string(), handle.clone());
        ensure_reaper();
        Ok(handle)
    }

    /// One-time, idempotent in-distro setup. Skips if the sentinel is present.
    async fn provision_distro(&self, distro: &str) -> Result<(), AppError> {
        if wsl_test_f(distro, PROVISION_SENTINEL).await {
            return Ok(());
        }

        // Copy the bundled agent into the distro via its /mnt path. `wslpath`
        // (run in-distro) does the Windows→/mnt translation robustly (handles
        // spaces, drive case).
        let agent_host = Self::agent_host_path();
        if !agent_host.exists() {
            return Err(AppError::internal_error(format!(
                "bundled sandbox agent not found at {} (set ZIEE_SANDBOX_AGENT)",
                agent_host.display()
            )));
        }
        let agent_mnt = wslpath(distro, &agent_host).await?;

        // Heredoc-free single script (passed via `bash -c`) so quoting stays sane.
        // WIN-TODO: prefer baking `bubblewrap` into the flavor recipe's
        // APT_PACKAGES so the apt step (network-dependent, slow) disappears.
        let script = format!(
            r#"set -e
command -v bwrap >/dev/null 2>&1 || {{ apt-get update -qq && apt-get install -y -qq bubblewrap; }}
install -m 0755 '{agent_mnt}' '{agent}'
printf '%s' "$ZIEE_PASSWD" > '{passwd}'
printf '%s' "$ZIEE_GROUP"  > '{group}'
# Unprivileged user namespaces — OFF by default in WSL2, AppArmor-restricted on
# noble. bwrap --unshare-user needs them. Set live + persist for re-boots.
sysctl -w kernel.unprivileged_userns_clone=1 2>/dev/null || true
sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 2>/dev/null || true
mkdir -p /etc/sysctl.d
{{ echo 'kernel.unprivileged_userns_clone=1'; echo 'kernel.apparmor_restrict_unprivileged_userns=0'; }} > /etc/sysctl.d/99-ziee-sandbox.conf
touch '{sentinel}'
"#,
            agent_mnt = agent_mnt,
            agent = GUEST_AGENT_PATH,
            passwd = GUEST_PASSWD,
            group = GUEST_GROUP,
            sentinel = PROVISION_SENTINEL,
        );

        // Pass the identity contents via env (translated into the distro through
        // WSLENV) to avoid embedding them in the script's quoting.
        let status = tokio::process::Command::new("wsl.exe")
            .args(["-d", distro, "-u", "root", "--", "bash", "-c", &script])
            .env("ZIEE_PASSWD", SYNTHETIC_PASSWD)
            .env("ZIEE_GROUP", SYNTHETIC_GROUP)
            .env("WSLENV", "ZIEE_PASSWD:ZIEE_GROUP")
            .status()
            .await
            .map_err(|e| AppError::internal_error(format!("run WSL2 provision: {e}")))?;
        if !status.success() {
            return Err(AppError::internal_error(format!(
                "WSL2 provision failed (exit {:?}) for {distro}",
                status.code()
            )));
        }
        Ok(())
    }
}

/// Translate a Windows host workspace path to its in-distro `/mnt/<drive>` path
/// (e.g. `C:\Users\me\ws\<conv>` → `/mnt/c/Users/me/ws/<conv>`).
///
/// WIN-TODO (perf): `/mnt/<drive>` is 9p (slow for many small files — the R/pip
/// workload). The Plan §3 follow-up is to relocate the workspace onto the
/// distro's ext4 and relay file contents via `tools/files.rs`; for the first
/// cut we bind the 9p path, which is correct if slower.
fn win_to_wsl_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("/mnt/{}{}", drive, &s[2..]);
    }
    s
}

#[async_trait]
impl SandboxBackend for Wsl2Backend {
    fn probe_host(&self, _cfg: &CodeSandboxConfig) -> Option<HostCapabilities> {
        // Cheap host-only probe (sub-10 ms): wsl.exe must be on PATH and the
        // default version must be 2 — bwrap needs the WSL2 Linux kernel
        // (cgroup v2, real namespaces); WSL1 is a syscall-emulation layer that
        // has none of these and will never work. We deliberately do NOT import
        // a distro / flip sysctls here — that's `ensure_rootfs_ready`'s job on
        // first `execute_command`, lazy by design.
        let out = match std::process::Command::new("wsl.exe").args(["--status"]).output() {
            Ok(o) => o,
            Err(e) => {
                tracing::error!(
                    "code_sandbox: wsl.exe not found ({e}); sandbox MCP row \
                     will NOT be registered. Install WSL2: `wsl --install`."
                );
                return None;
            }
        };
        let status_text = decode_wsl_output(&out.stdout);
        if !status_text.contains('2') {
            // `wsl --status` includes the default-version line ("Default Version: 2").
            // If we can't confirm v2, refuse loudly — bwrap will never work on v1.
            tracing::error!(
                "code_sandbox: WSL2 not the default version on this host; \
                 sandbox MCP row will NOT be registered. Set with \
                 `wsl --set-default-version 2`. Probe output: {status_text:?}"
            );
            return None;
        }
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
        // Shared fetch coordination (OS-independent), but the WINDOWS packaging:
        // the `.tar.zst` that `wsl --import` consumes (Plan 1 §4).
        let cache = cache_dir(state);
        let outcome =
            runtime_fetch::ensure_fetched_format(&cache, flavor, RootfsFormat::TarZst, |_| {})
                .await
                .map_err(|e| AppError::internal_error(format!("rootfs fetch failed: {e}")))?;

        let guest_caps = HardeningCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            pid_namespace: PidNsMode::Strict,
            // The guest agent applies cgroup v2 from ExecRequest.cgroup (recent
            // WSL2 ships cgroup v2); prlimit in the argv is the backstop. See run().
            cgroup: CgroupMode::None,
            // The agent builds + applies the shared seccomp filter itself.
            seccomp: SeccompMode::NotLinked,
        };
        Ok(EnsureOutcome {
            caps: Arc::new(guest_caps),
            // The imported distro filesystem IS the rootfs ⇒ root is "/".
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
        // Locate (fetch if needed) the flavor tarball; idempotent on cache hit.
        let cache = cache_dir(state);
        let tarball =
            runtime_fetch::ensure_fetched_format(&cache, flavor, RootfsFormat::TarZst, |_| {})
                .await
                .map_err(|e| AppError::internal_error(format!("rootfs fetch failed: {e}")))?
                .installed_path;

        // Build the bwrap argv with GUEST paths so the agent execs it verbatim —
        // identical hardening to the Linux/macOS backends.
        let guest_caps = HardeningCapabilities {
            bwrap_path: PathBuf::from(GUEST_BWRAP_PATH),
            pid_namespace: PidNsMode::Strict,
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        };
        let guest_ctx = SandboxContext {
            conversation_id: ctx.conversation_id,
            user_id: ctx.user_id,
            // The workspace lives on the Windows host; bind its /mnt path in-distro.
            workspace: PathBuf::from(win_to_wsl_path(&ctx.workspace)),
            files: ctx.files.clone(),
        };
        let secs = timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
        let req = ExecRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: REQ_COUNTER.fetch_add(1, Ordering::Relaxed),
            bwrap_path: GUEST_BWRAP_PATH.to_string(),
            argv: sandbox::build_bwrap_argv(
                &guest_caps,
                &guest_ctx.workspace,
                &guest_ctx,
                Path::new(GUEST_ROOTFS_MOUNT),
                command,
                Path::new(GUEST_PASSWD),
                Path::new(GUEST_GROUP),
                Some(GUEST_SECCOMP_FD),
            ),
            timeout_ms: secs * 1000,
            seccomp_fd: Some(GUEST_SECCOMP_FD),
            // In-guest cgroup v2 (the agent applies it; prlimit is the backstop).
            // WIN-TODO: if the WSL2 kernel lacks delegated cgroup v2 the agent
            // should degrade gracefully to rlimits-only. Source from §6 config.
            cgroup: Some(CgroupLimits::default_policy()),
        };

        // Up to 2 attempts: a dead/unreachable distro (connect fails — the
        // command never ran, so retry is safe) is evicted + re-booted once
        // (mirrors macOS B1). A failure AFTER connect is NOT retried.
        let mut attempt = 0;
        loop {
            attempt += 1;
            let h = self.ensure_distro(state, flavor, &tarball).await?;
            let _permit = h.sem.acquire().await.expect("distro semaphore never closed");
            h.inflight.fetch_add(1, Ordering::SeqCst);
            let _guard = InflightGuard(h.clone());
            *h.last_used.lock().await = Instant::now();

            match TcpStream::connect(("127.0.0.1", h.tcp_port)).await {
                Ok(stream) => {
                    let result = super::vm_client::run_on_stream(stream, req.clone(), secs).await;
                    *h.last_used.lock().await = Instant::now();
                    return result;
                }
                Err(e) if attempt < 2 => {
                    tracing::warn!(
                        flavor,
                        "code_sandbox: WSL2 agent unreachable ({e}); re-booting and retrying"
                    );
                    drop(_guard);
                    drop(_permit);
                    evict_dead_distro(flavor, &h).await;
                    continue;
                }
                Err(e) => {
                    return Err(AppError::internal_error(format!(
                        "connect WSL2 agent: {e}"
                    )));
                }
            }
        }
    }

    async fn shutdown(&self) {
        let mut distros = DISTROS.lock().await;
        for (flavor, h) in distros.drain() {
            stop_agent(&h).await;
            let _ = run_wsl(&["--terminate", &h.distro]).await;
            tracing::info!(
                flavor,
                distro = %h.distro,
                "code_sandbox: WSL2 distro stopped on shutdown"
            );
        }
    }

    async fn evict_flavor(&self, cache_dir: &Path, flavor: &str) -> EvictOutcome {
        // Stop + unregister the distro if running/registered.
        if let Some(h) = DISTROS.lock().await.remove(flavor) {
            stop_agent(&h).await;
            let _ = run_wsl(&["--unregister", &h.distro]).await;
        } else {
            // Not warm but may still be registered from a prior run.
            let distro = Self::distro_name(flavor);
            if distro_registered(&distro).await {
                let _ = run_wsl(&["--unregister", &distro]).await;
            }
        }
        // Delete the cached tarball for this flavor.
        let suffix = format!("-{flavor}.tar.zst");
        let mut bytes_freed = 0;
        let mut was_cached = false;
        if let Ok(rd) = std::fs::read_dir(cache_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(&suffix))
                {
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

/// Kill the agent relay child for a handle.
async fn stop_agent(h: &DistroHandle) {
    let mut agent = h.agent.lock().await;
    let _ = agent.start_kill();
    let _ = agent.wait().await;
}

/// Remove a dead/unreachable distro from the registry (only if it's still the
/// current handle for the flavor) and stop its agent, so the next
/// `ensure_distro` re-boots (mirrors macOS B1). Idempotent.
async fn evict_dead_distro(flavor: &str, dead: &Arc<DistroHandle>) {
    {
        let mut distros = DISTROS.lock().await;
        if distros.get(flavor).is_some_and(|h| Arc::ptr_eq(h, dead)) {
            distros.remove(flavor);
        }
    }
    stop_agent(dead).await;
    // Terminate (not unregister) so a re-boot reuses the imported filesystem.
    let _ = run_wsl(&["--terminate", &dead.distro]).await;
}

/// `true` if `distro` appears in `wsl.exe -l -q`. WSL emits UTF-16LE, so decode
/// it before matching.
async fn distro_registered(distro: &str) -> bool {
    match tokio::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .output()
        .await
    {
        Ok(out) => decode_wsl_output(&out.stdout)
            .lines()
            .any(|l| l.trim() == distro),
        Err(_) => false,
    }
}

/// `test -f <path>` inside the distro (idempotency probe for provisioning).
async fn wsl_test_f(distro: &str, path: &str) -> bool {
    tokio::process::Command::new("wsl.exe")
        .args(["-d", distro, "-u", "root", "--", "test", "-f", path])
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run `wslpath -a <winpath>` inside the distro to get the `/mnt/...` path.
async fn wslpath(distro: &str, win: &Path) -> Result<String, AppError> {
    let out = tokio::process::Command::new("wsl.exe")
        .args(["-d", distro, "-u", "root", "--", "wslpath", "-a"])
        .arg(win.as_os_str())
        .output()
        .await
        .map_err(|e| AppError::internal_error(format!("wslpath: {e}")))?;
    if !out.status.success() {
        return Err(AppError::internal_error(format!(
            "wslpath failed for {}",
            win.display()
        )));
    }
    Ok(decode_wsl_output(&out.stdout).trim().to_string())
}

/// Run a top-level `wsl.exe <args>` (no `-d`), erroring on non-zero exit.
async fn run_wsl(args: &[&str]) -> Result<(), AppError> {
    let out = tokio::process::Command::new("wsl.exe")
        .args(args)
        .output()
        .await
        .map_err(|e| AppError::internal_error(format!("wsl.exe {args:?}: {e}")))?;
    if !out.status.success() {
        return Err(AppError::internal_error(format!(
            "wsl.exe {args:?} failed: {}",
            decode_wsl_output(&out.stderr)
        )));
    }
    Ok(())
}

/// WSL's top-level commands (`-l`, `--status`) emit UTF-16LE; in-distro command
/// stdout is UTF-8. Decode UTF-16LE when it looks like it (alternating
/// NUL-byte pattern), else fall back to lossy UTF-8.
fn decode_wsl_output(bytes: &[u8]) -> String {
    let looks_utf16 = bytes.len() >= 4
        && bytes.len() % 2 == 0
        && bytes
            .iter()
            .skip(1)
            .step_by(2)
            .take(8)
            .filter(|&&b| b == 0)
            .count()
            >= 2;
    if looks_utf16 {
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}
