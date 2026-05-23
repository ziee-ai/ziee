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
//!     passwd/group, and installs a **narrow AppArmor profile** that grants
//!     `userns` only to `/usr/bin/bwrap` — leaving Ubuntu 24.04/noble's
//!     `kernel.apparmor_restrict_unprivileged_userns` defense in place for
//!     everything else. The profile + sysctls are re-applied on every VM boot
//!     via `/etc/wsl.conf` `[boot] command =` (WSL has no systemd by default;
//!     [microsoft/WSL#4232]).
//!   - The agent is started inside the distro listening on **127.0.0.1:<port>**;
//!     WSL2's localhost-forwarding makes that reachable from Windows. Per
//!     `execute_command` this backend connects a `TcpStream` and sends the
//!     bwrap argv (guest paths) — the agent applies the shared seccomp + cgroup
//!     in-guest, identical to macOS.
//!
//! ## Threat model (cross-platform delta with the Linux backend)
//!
//!   - **Cross-distro reachability on the TCP transport (HIGH-1, tracked):**
//!     WSL2 distros share **one network namespace** in the utility VM
//!     ([microsoft/WSL#4304], `init/main.cpp:2283` — no `CLONE_NEWNET`). Until
//!     the planned AF_VSOCK switch, every other WSL2 distro the user has
//!     installed (their personal Ubuntu, Docker Desktop's distros, …) can
//!     reach our agent on `127.0.0.1:<port>` and submit an arbitrary bwrap
//!     argv. The TCP listener binds `127.0.0.1` (not `0.0.0.0`), so LAN-side
//!     attack is blocked; the gap is intra-VM only.
//!   - **`networkingMode = mirrored` collapses host↔guest loopback:** in
//!     mirrored mode (Win11 22H2+) the Windows host's `127.0.0.1` services
//!     (Postgres, the Ziee server itself, …) are reachable from inside the
//!     sandbox because of `--share-net`. `probe_host` warn-logs when this
//!     mode is detected in `%USERPROFILE%\.wslconfig`.
//!   - **9p `/mnt/<drive>` workspace bind:** the WSL2 kernel's plan9 client
//!     is an attack surface from inside the sandbox (see [McAfee Labs WSL
//!     Plan 9 BSOD research] + [CVE-2026-43053]). MED-1 follow-up moves the
//!     workspace into the distro's ext4 and ferries file content over the
//!     control plane; out-of-scope for the first cut, documented at
//!     `win_to_wsl_path`.
//!
//! ⚠️ **Validation status:** this file cannot be compiled or run on Linux (it is
//! `cfg(target_os = "windows")`). It is grounded in the documented WSL2 + agent
//! behavior + the prior-art deep read in `.sec-audits/`, but must be compiled
//! and validated on Windows 11 + WSL2. Points flagged `WIN-TODO` need
//! first-run attention.

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
// Bumped to v2 when the provisioning surface changed (narrow AppArmor profile +
// /etc/wsl.conf [boot] command, replacing the global apparmor_restrict_*=0
// sysctl). Older distros that have the v1 sentinel will be re-provisioned on
// the next boot — the v1 state is otherwise indistinguishable.
const PROVISION_SENTINEL: &str = "/etc/ziee-sandbox-provisioned-v2";

const GUEST_APPARMOR_PROFILE: &str = "/etc/apparmor.d/bwrap";
const GUEST_SYSCTL_CONF: &str = "/etc/sysctl.d/99-ziee-sandbox.conf";
const GUEST_WSL_CONF: &str = "/etc/wsl.conf";

/// Narrow AppArmor profile (Claude-Code-documented recipe, verified in code
/// during the prior-art deep read). Grants `userns` ONLY to `/usr/bin/bwrap`,
/// leaving Ubuntu 24.04/noble's `kernel.apparmor_restrict_unprivileged_userns`
/// in force for every other binary. The earlier wsl2 implementation flipped
/// that sysctl to `0` globally — that disables Ubuntu's documented mitigation
/// against in-userns kernel exploits for the WHOLE distro, including any
/// sandbox-escaped process. This profile keeps the kernel-level restriction
/// and only carves out our explicit bwrap binary.
const APPARMOR_BWRAP_PROFILE: &str = "abi <abi/4.0>,\n\
include <tunables/global>\n\
\n\
profile bwrap /usr/bin/bwrap flags=(unconfined) {\n\
  userns,\n\
  include if exists <local/bwrap>\n\
}\n";

/// `/etc/wsl.conf` we install. WSL has no systemd by default, so the standard
/// systemd-sysctl / apparmor.service paths don't run on boot — sysctls in
/// `/etc/sysctl.d/` are NEVER re-applied ([microsoft/WSL#4232], open since
/// 2019), and any AppArmor profile we wrote is unloaded on `wsl --shutdown`.
/// `[boot] command` IS reliably run as root via `execl("/bin/sh", "sh", "-c",
/// Command, …)` (`microsoft/WSL: src/linux/init/config.cpp:1002`) on every VM
/// boot — that's where the re-apply lives.
const WSL_CONF_CONTENT: &str = "# Managed by ziee-sandbox (Plan 1 §3) — DO NOT EDIT.\n\
# Re-applies the narrow bwrap AppArmor profile + sysctls on every VM boot.\n\
# WSL2 has no systemd by default, so /etc/sysctl.d/* and /etc/apparmor.d/* are\n\
# otherwise NOT applied on boot (microsoft/WSL#4232).\n\
[boot]\n\
command = apparmor_parser -r /etc/apparmor.d/bwrap 2>/dev/null || apparmor_parser /etc/apparmor.d/bwrap 2>/dev/null || true; sysctl --system >/dev/null 2>&1 || true\n";

const SYSCTL_CONF_CONTENT: &str = "# Managed by ziee-sandbox (Plan 1 §3) — DO NOT EDIT.\n\
# The Ubuntu/Debian downstream sysctl that enables unprivileged user-namespace\n\
# creation. May be a no-op on the Microsoft kernel build (which can have it\n\
# always-on); the `2>/dev/null` in the wsl.conf reload makes that case silent.\n\
kernel.unprivileged_userns_clone=1\n";

/// Minimum WSL versions free of CVE-2025-53788 (TOCTOU LPE in the WSL2 kernel
/// code, fixed in 2.5.10 on the 2.5 channel and 2.6.1 on the 2.6 channel).
/// Anything older lets unprivileged code inside the sandbox race the kernel to
/// SYSTEM on the Windows host — we refuse to register.
const WSL_MIN_VERSION_25: (u32, u32, u32) = (2, 5, 10);
const WSL_MIN_VERSION_26: (u32, u32, u32) = (2, 6, 1);

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
                    // `wsl --terminate <distro>` stops THIS distro's in-VM init
                    // but does NOT free the shared WSL2 utility VM's RAM — that
                    // requires `wsl --shutdown` (which also kills every other
                    // distro the user has running, so we don't do it here). The
                    // value of `--terminate` is bounding the agent + tearing
                    // down anything the distro side was holding open; cached
                    // page memory in the utility VM only frees if the user
                    // separately runs `wsl --shutdown`. See microsoft/WSL FAQ
                    // + #13291.
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
    ///
    /// Steps (each idempotent on its own — a partial provision rerunning is
    /// safe):
    ///   1. Install the `ziee-sandbox-agent` Linux binary at `GUEST_AGENT_PATH`.
    ///   2. Write the synthetic passwd / group files (via stdin pipes — no
    ///      `WSLENV`, no env crossing the boundary — MED-3).
    ///   3. Write the narrow `/etc/apparmor.d/bwrap` profile (HIGH-2).
    ///   4. Write `/etc/sysctl.d/99-ziee-sandbox.conf` + `/etc/wsl.conf`
    ///      `[boot] command =` so the AppArmor profile + sysctl re-apply on
    ///      every VM boot (HIGH-4 — fixes the [microsoft/WSL#4232] silent
    ///      breakage where sysctls don't re-apply across `wsl --shutdown`).
    ///   5. `apt-get install bubblewrap` if absent.
    ///   6. Apply the AppArmor profile + sysctls LIVE in this boot too (the
    ///      wsl.conf hook handles every SUBSEQUENT boot).
    ///   7. Write the sentinel.
    async fn provision_distro(&self, distro: &str) -> Result<(), AppError> {
        if wsl_test_f(distro, PROVISION_SENTINEL).await {
            return Ok(());
        }

        // 1. Agent binary. Copy via `wslpath` so quoting / drive-case / spaces
        //    are handled inside the distro.
        let agent_host = Self::agent_host_path();
        if !agent_host.exists() {
            return Err(AppError::internal_error(format!(
                "bundled sandbox agent not found at {} (set ZIEE_SANDBOX_AGENT)",
                agent_host.display()
            )));
        }
        let agent_mnt = wslpath(distro, &agent_host).await?;
        run_in_distro(
            distro,
            &format!("install -m 0755 '{agent_mnt}' '{GUEST_AGENT_PATH}'"),
        )
        .await?;

        // 2. Synthetic identity, content piped via stdin. Replaces the earlier
        //    WSLENV approach — no env vars cross from Windows. A future
        //    maintainer adding a `WSLENV=…` would silently leak; this design
        //    closes that footgun.
        write_file_into_distro(distro, GUEST_PASSWD, SYNTHETIC_PASSWD, 0o644).await?;
        write_file_into_distro(distro, GUEST_GROUP, SYNTHETIC_GROUP, 0o644).await?;

        // 3 + 4. AppArmor profile + sysctl.d + wsl.conf (re-apply on boot).
        run_in_distro(distro, "mkdir -p /etc/apparmor.d /etc/sysctl.d").await?;
        write_file_into_distro(distro, GUEST_APPARMOR_PROFILE, APPARMOR_BWRAP_PROFILE, 0o644).await?;
        write_file_into_distro(distro, GUEST_SYSCTL_CONF, SYSCTL_CONF_CONTENT, 0o644).await?;
        write_file_into_distro(distro, GUEST_WSL_CONF, WSL_CONF_CONTENT, 0o644).await?;

        // 5. Install bwrap (only step that needs network + can be slow).
        //    WIN-TODO: prefer baking `bubblewrap` into the flavor recipe's
        //    APT_PACKAGES so this step disappears in v3 of the rootfs schema.
        run_in_distro(
            distro,
            "command -v bwrap >/dev/null 2>&1 || \
             { apt-get update -qq && apt-get install -y -qq bubblewrap; }",
        )
        .await?;

        // 6. Apply LIVE for this boot too. `apparmor_parser -r` reloads if
        //    already loaded; plain `apparmor_parser` adds a new profile.
        //    Either-or-true keeps provision idempotent across both states.
        run_in_distro(
            distro,
            "apparmor_parser -r /etc/apparmor.d/bwrap 2>/dev/null \
             || apparmor_parser /etc/apparmor.d/bwrap 2>/dev/null || true; \
             sysctl --system >/dev/null 2>&1 || true",
        )
        .await?;

        // 7. Sentinel.
        run_in_distro(distro, &format!("touch '{PROVISION_SENTINEL}'")).await?;
        Ok(())
    }
}

/// Run a single bash command inside `distro` as root, erroring on non-zero exit.
async fn run_in_distro(distro: &str, script: &str) -> Result<(), AppError> {
    let status = tokio::process::Command::new("wsl.exe")
        .args(["-d", distro, "-u", "root", "--", "bash", "-c", script])
        .status()
        .await
        .map_err(|e| AppError::internal_error(format!("wsl bash -c: {e}")))?;
    if !status.success() {
        return Err(AppError::internal_error(format!(
            "in-distro command failed (exit {:?}): {}",
            status.code(),
            // Truncate so a multiline AppArmor profile / wsl.conf doesn't
            // dominate the error.
            script.chars().take(80).collect::<String>()
        )));
    }
    Ok(())
}

/// Write `content` to `dest_path` inside `distro` via stdin (no env crossings,
/// no temp files on the Windows host, no quoting issues with newlines / shell
/// metacharacters). Replaces the earlier `WSLENV=ZIEE_PASSWD:ZIEE_GROUP`
/// pattern that risked leaking any future credential added there.
async fn write_file_into_distro(
    distro: &str,
    dest_path: &str,
    content: &str,
    mode: u32,
) -> Result<(), AppError> {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;
    let script = format!(
        "umask 077 && cat > '{dest_path}' && chmod {mode:o} '{dest_path}'"
    );
    let mut child = Command::new("wsl.exe")
        .args(["-d", distro, "-u", "root", "--", "bash", "-c", &script])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::internal_error(format!("spawn wsl write {dest_path}: {e}")))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::internal_error("wsl stdin missing"))?;
        stdin
            .write_all(content.as_bytes())
            .await
            .map_err(|e| AppError::internal_error(format!("pipe {dest_path}: {e}")))?;
        // Drop closes stdin → cat sees EOF → exits → bash chmods → exits.
    }
    let status = child
        .wait()
        .await
        .map_err(|e| AppError::internal_error(format!("wait wsl write {dest_path}: {e}")))?;
    if !status.success() {
        return Err(AppError::internal_error(format!(
            "wsl write {dest_path} failed (exit {:?})",
            status.code()
        )));
    }
    Ok(())
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
        // Cheap host-only probe (sub-10 ms): wsl.exe must be on PATH, the
        // default version must be 2 (bwrap needs the WSL2 Linux kernel; WSL1
        // is syscall emulation with no namespaces and never works), AND the
        // WSL runtime must be patched against CVE-2025-53788 (TOCTOU LPE in
        // the WSL2 kernel code, ≥ 2.5.10 / 2.6.1). We deliberately do NOT
        // import a distro / flip sysctls / load AppArmor here — that's
        // `ensure_rootfs_ready`'s job on first `execute_command`, lazy by design.

        // 1. v2-default check.
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
            tracing::error!(
                "code_sandbox: WSL2 not the default version on this host; \
                 sandbox MCP row will NOT be registered. Set with \
                 `wsl --set-default-version 2`. Probe output: {status_text:?}"
            );
            return None;
        }

        // 2. CVE-2025-53788 version gate.
        match probe_wsl_version() {
            Ok(v) => {
                if !wsl_version_is_patched(v) {
                    tracing::error!(
                        wsl_version = ?v,
                        "code_sandbox: WSL runtime is older than the \
                         CVE-2025-53788 fix (need {:?} on the 2.5 channel or \
                         {:?} on the 2.6 channel). Run `wsl --update`. \
                         Sandbox MCP row will NOT be registered.",
                        WSL_MIN_VERSION_25, WSL_MIN_VERSION_26
                    );
                    return None;
                }
            }
            Err(e) => {
                // Old WSL releases don't support `wsl --version` at all. Treat
                // as unpatched and refuse.
                tracing::error!(
                    "code_sandbox: could not determine WSL version ({e}); \
                     `wsl --version` may be missing on pre-2.0 WSL. Run \
                     `wsl --update`. Sandbox MCP row will NOT be registered."
                );
                return None;
            }
        }

        // 3. LOW-3: warn-log on mirrored networking mode. `--share-net` already
        //    bridges egress; mirrored mode additionally collapses the Windows
        //    host's `127.0.0.1` services (Postgres, the Ziee server itself, …)
        //    into the sandbox's reach. Operators should know.
        if user_wslconfig_uses_mirrored_mode() {
            tracing::warn!(
                "code_sandbox: WSL2 mirrored networking mode is enabled in \
                 .wslconfig. Sandboxed commands can reach the Windows host's \
                 127.0.0.1 services (Postgres, the Ziee server, …) via \
                 `--share-net`. Consider switching back to NAT mode or \
                 firewalling sensitive host services."
            );
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

/// Parse the first non-blank line of `wsl --version` for a (major, minor,
/// patch) triple. The Windows binary prints e.g.:
///   `WSL version: 2.6.1.0`
///   `Kernel version: 6.6.87.2-1`
/// The fourth component (`.0` build) is ignored. Old WSL versions don't
/// implement `--version` and exit non-zero — that case bubbles up as Err.
fn probe_wsl_version() -> Result<(u32, u32, u32), String> {
    let out = std::process::Command::new("wsl.exe")
        .args(["--version"])
        .output()
        .map_err(|e| format!("wsl --version: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "wsl --version failed: {}",
            decode_wsl_output(&out.stderr).trim()
        ));
    }
    let text = decode_wsl_output(&out.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Match the version line robustly across localized prefixes ("WSL
        // version:", "Versión de WSL:", …) by scanning each line for a
        // dotted-integer triple.
        if let Some(v) = extract_version_triple(line) {
            return Ok(v);
        }
    }
    Err(format!("no version triple found in: {text:?}"))
}

fn extract_version_triple(line: &str) -> Option<(u32, u32, u32)> {
    // Find a substring that looks like `<u32>.<u32>.<u32>` (allow trailing
    // `.<build>` which we ignore).
    let mut chars = line.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if !c.is_ascii_digit() {
            continue;
        }
        let mut end = start;
        for (i, ch) in line[start..].char_indices() {
            if ch.is_ascii_digit() || ch == '.' {
                end = start + i + ch.len_utf8();
            } else {
                break;
            }
        }
        let candidate = &line[start..end];
        let mut parts = candidate.split('.');
        if let (Some(a), Some(b), Some(c), _) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        {
            if let (Ok(major), Ok(minor), Ok(patch)) = (a.parse(), b.parse(), c.parse()) {
                return Some((major, minor, patch));
            }
        }
    }
    None
}

/// CVE-2025-53788 patched-version gate. Fix landed on two release channels:
/// 2.5.10 (the older one) and 2.6.1. Anything on either channel at or above
/// its fix is acceptable; anything below either is rejected.
fn wsl_version_is_patched((major, minor, patch): (u32, u32, u32)) -> bool {
    if major != 2 {
        // 1.x / 3+: out of scope; we already rejected by the v2 check.
        return major > 2;
    }
    match minor {
        // ≥ 2.6 series: 2.6.1+.
        m if m >= WSL_MIN_VERSION_26.1 => {
            (minor, patch) >= (WSL_MIN_VERSION_26.1, WSL_MIN_VERSION_26.2)
        }
        // 2.5 series: 2.5.10+.
        m if m == WSL_MIN_VERSION_25.1 => patch >= WSL_MIN_VERSION_25.2,
        // < 2.5: not patched.
        _ => false,
    }
}

/// `true` if `%USERPROFILE%\.wslconfig` has `networkingMode = mirrored` in
/// some form. Best-effort tokenizer (no full INI parser needed for one knob).
fn user_wslconfig_uses_mirrored_mode() -> bool {
    let userprofile = match std::env::var("USERPROFILE") {
        Ok(p) => p,
        Err(_) => return false,
    };
    let path = std::path::Path::new(&userprofile).join(".wslconfig");
    let text = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("networkingmode") && lower.contains("mirrored") {
            return true;
        }
    }
    false
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
