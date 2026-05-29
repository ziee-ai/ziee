//! Per-flavor lazy rootfs mount + sole owner of every squashfuse FUSE daemon.
//!
//! `init()` does NOT mount any rootfs — it only probes host
//! capabilities (bwrap, cgroup, seccomp) and registers the MCP row.
//! Squashfuse spawn + rootfs-dependent probes are deferred until the
//! first `execute_command` for each flavor hits [`ensure_rootfs_ready`].
//! Users who never invoke code execution pay zero FUSE-process cost.
//! Users who only use `minimal` never spawn squashfuse for `full`.
//!
//! ## Per-flavor isolation
//!
//! Each flavor has its own squashfs file in the cache dir (filename
//! ends with `-{flavor}.squashfs`), its own mount directory (derived
//! from the squashfs filename stem, so naturally distinct per
//! flavor), and its own [`MountedRootfs`] entry in [`MOUNTED`]. Two
//! flavors can be mounted simultaneously; bwrap calls bind whichever
//! one the LLM picked.
//!
//! ## Lifecycle
//!
//! - **First call** to `ensure_rootfs_ready(state, flavor)`:
//!   - If the cached squashfs for `flavor` is already mounted at its
//!     expected mount dir (e.g. pre-mounted by `just sandbox-mount`
//!     or by a previous server run that didn't clean up), reuse it.
//!   - Else: find the `.squashfs` for `flavor` in the cache dir,
//!     spawn `squashfuse -f` with `PR_SET_PDEATHSIG=SIGTERM`, poll
//!     for the mount to appear.
//!   - Read the version sentinel (informational only, post Plan 5
//!     Phase 2) + run `probe_pid_ns` against the flavor-specific
//!     mount. Cache the resulting `HardeningCapabilities` in
//!     `READY[<version>/<flavor>]`.
//! - **Subsequent calls for the same flavor**: atomic load from the
//!   per-flavor `OnceCell`. Cached failure is sticky.
//! - **Subsequent calls for a different flavor**: spawn squashfuse
//!   for THAT flavor, mount alongside. Both stay live.
//! - **Server shutdown** ([`shutdown`]): iterate the MOUNTED map,
//!   kill every Child + `fusermount -u` each mount dir.
//! - **Force-quit (SIGKILL)**: PDEATHSIG delivers SIGTERM to every
//!   squashfuse child. Each unmounts itself. No app cooperation.

use std::collections::HashMap;
// Linux-only: `pre_exec` (PDEATHSIG) on the squashfuse child. Gated so the
// crate compiles on macOS/Windows, where mounting happens in the VM / WSL2
// backend rather than via a host squashfuse process.
// (CommandExt brings pre_exec onto std::process::Command, but the
// runtime_mount path no longer uses pre_exec — squashfuse is spawned
// via tokio::process. Removed to satisfy clippy.)
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use dashmap::DashMap;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, OnceCell};

use crate::common::r#type::AppError;
use crate::modules::code_sandbox::runtime_fetch::{
    self, FetchError, FetchOutcome, FetchPhase,
};
use crate::modules::code_sandbox::types::{CodeSandboxState, HardeningCapabilities};

// =====================================================================
// Per-flavor lazy-init state
// =====================================================================

type ReadyResult = Result<EnsureOutcome, ReadyError>;
type ReadyCell = Arc<OnceCell<ReadyResult>>;

/// Outer OnceCell wraps the DashMap (init the map once). Inner
/// per-flavor `OnceCell` ensures a single squashfuse spawn per flavor
/// even under concurrent first calls; subsequent calls await the
/// same cell.
static READY: OnceCell<DashMap<String, ReadyCell>> = OnceCell::const_new();

/// Squashfuse processes WE spawned, keyed by flavor. On shutdown we
/// iterate this map and kill+unmount each one.
static MOUNTED: OnceCell<Mutex<HashMap<String, MountedRootfs>>> = OnceCell::const_new();

struct MountedRootfs {
    child: Child,
    mount_dir: PathBuf,
}

/// What `ensure_rootfs_ready` returns. The caller (`run_in_sandbox`)
/// uses `caps` for build_bwrap_argv and `mount_dir` for the
/// `--ro-bind` source.
///
/// `fetch_info` is populated when THIS call did the auto-fetch; the
/// chat UI uses it to render a "fetched X (Y MB in Z s)" system
/// note. `None` when the flavor's squashfs was already in the cache.
///
/// `artifact_id` identifies the row in `code_sandbox_rootfs_artifacts`
/// that this mount corresponds to. Callers that wish to participate
/// in the drain-on-swap protocol (Plan 5 Phase 3) acquire an
/// `InflightGuard` against it via
/// `version_manager::acquire_inflight(artifact_id, kind)`.
#[derive(Debug, Clone)]
pub struct EnsureOutcome {
    pub caps: Arc<HardeningCapabilities>,
    pub mount_dir: PathBuf,
    pub fetch_info: Option<FetchOutcome>,
    pub artifact_id: Option<uuid::Uuid>,
    pub artifact_version: Option<String>,
}

// =====================================================================
// Errors (mapped to structured AppError responses for MCP)
// =====================================================================

#[derive(Debug, Clone)]
pub enum ReadyError {
    SquashfuseMissing,
    NoRootfsForFlavor { flavor: String, cache_dir: PathBuf },
    FetchFailed { flavor: String, reason: String },
    MountFailed { reason: String },
    PidNsDisabled { reason: String },
    // ── VM-backend lazy-init failures (Plan 1 §5) ──
    // Cross-platform but currently only constructed on macOS/Windows; kept on
    // all builds so a stray `match` is total.
    /// `wsl.exe` is absent from PATH (no WSL installed).
    Wsl2NotPresent,
    /// Only WSL v1 distros are available; bwrap needs WSL v2's Linux kernel.
    Wsl1Refused,
    /// The provisioned WSL distro can't enable unprivileged user namespaces —
    /// even after writing the sysctls, the kernel/AppArmor profile blocks them.
    UsernsDisabledInWsl,
    /// A libkrun (macOS) / wsl.exe (Windows) microVM failed to boot within the
    /// deadline (`reason` carries the specific cause).
    VmBootFailed { reason: String },
    /// libkrun's dylib could not be loaded by the macOS launcher (the dep
    /// wasn't bundled, or the runtime linker can't find it).
    LibkrunMissing,
}

impl ReadyError {
    fn to_app_error(&self) -> AppError {
        match self {
            ReadyError::SquashfuseMissing => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_SQUASHFUSE_MISSING",
                "sandbox cannot start: squashfuse is not installed. \
                 Install it (e.g. `apt install squashfuse fuse3`) and restart.",
            ),
            ReadyError::NoRootfsForFlavor { flavor, cache_dir } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_ROOTFS_NOT_FETCHED",
                format!(
                    "sandbox cannot start: no rootfs artifact for flavor {flavor:?} in {}. \
                     This is normally auto-fetched on first use; the failure indicates \
                     either no network at startup or the pinned version is missing on GitHub.",
                    cache_dir.display()
                ),
            ),
            ReadyError::FetchFailed { flavor, reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_FETCH_FAILED",
                format!(
                    "sandbox cannot start: auto-fetch of flavor {flavor:?} failed: \
                     {reason}. Check network connectivity to GitHub Releases (or \
                     CODE_SANDBOX_ROOTFS_MIRROR if set)."
                ),
            ),
            ReadyError::MountFailed { reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_MOUNT_FAILED",
                format!("sandbox cannot start: rootfs mount failed: {reason}"),
            ),
            ReadyError::PidNsDisabled { reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_PIDNS_DISABLED",
                format!("sandbox cannot start: {reason}"),
            ),
            ReadyError::Wsl2NotPresent => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_WSL2_NOT_PRESENT",
                "sandbox cannot start: WSL is not installed on this Windows host. \
                 Install it with `wsl --install`.",
            ),
            ReadyError::Wsl1Refused => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_WSL1_REFUSED",
                "sandbox cannot start: WSL v1 detected; bwrap needs the WSL v2 \
                 Linux kernel. Run `wsl --set-default-version 2`.",
            ),
            ReadyError::UsernsDisabledInWsl => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_USERNS_DISABLED_IN_WSL",
                "sandbox cannot start: the WSL distro does not allow \
                 unprivileged user namespaces (bwrap --unshare-user). Either \
                 the kernel was built without CONFIG_USER_NS, or AppArmor is \
                 blocking unprivileged userns and provisioning could not \
                 disable it.",
            ),
            ReadyError::VmBootFailed { reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_VM_BOOT_FAILED",
                format!("sandbox cannot start: VM boot failed: {reason}"),
            ),
            ReadyError::LibkrunMissing => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_LIBKRUN_MISSING",
                "sandbox cannot start: libkrun dylib could not be loaded by \
                 the macOS VM launcher. Verify the app bundle includes \
                 libkrun.dylib in Contents/Frameworks.",
            ),
        }
    }
}

// =====================================================================
// Public entry: ensure the requested flavor is mounted + probed
// =====================================================================

pub async fn ensure_rootfs_ready(
    state: &CodeSandboxState,
    flavor: &str,
) -> Result<EnsureOutcome, AppError> {
    // Plan 5 Phase 2: the READY map is keyed on (version, flavor) so a
    // pin change invalidates the cached EnsureOutcome — the next call
    // mounts the new version against a fresh cell while live execs
    // continue to read the old cell's mount_dir from their captured
    // EnsureOutcome. Two pinned versions can coexist during drain.
    let pin = match state.pool.as_ref() {
        Some(pool) => {
            crate::modules::code_sandbox::version_manager::ensure_pin_initialized(pool)
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        }
        None => String::new(),
    };
    let key = if pin.is_empty() {
        flavor.to_string()
    } else {
        format!("{pin}/{flavor}")
    };

    let ready_map = READY.get_or_init(|| async { DashMap::new() }).await;
    let cell: ReadyCell = ready_map
        .entry(key.clone())
        .or_insert_with(|| Arc::new(OnceCell::new()))
        .clone();
    let cached = cell
        .get_or_init(|| async { do_first_init(state, flavor).await })
        .await;
    match cached {
        Ok(outcome) => Ok(outcome.clone()),
        Err(e) => {
            // L1: do NOT permanently cache a failed init. A transient failure
            // (fetch network blip, mount timeout) would otherwise wedge the
            // flavor until an admin evict. Drop the cell so the next call
            // re-inits (recovers when the network/mount recovers; a persistent
            // failure like a PID-ns probe failure just re-fails cheaply —
            // cache hit, mount skip). `remove_if` with identity guards
            // against clobbering a fresh cell another caller just inserted.
            ready_map.remove_if(&key, |_, v| Arc::ptr_eq(v, &cell));
            Err(e.to_app_error())
        }
    }
}

async fn do_first_init(state: &CodeSandboxState, flavor: &str) -> ReadyResult {
    let cache_dir = derive_cache_dir(state);

    // 0. Resolve + auto-fetch. `ensure_fetched` is idempotent: a warm
    //    cache hit short-circuits without touching the network, so we
    //    don't need a separate fast-path here. The returned outcome
    //    carries the pinned `version` plus stats for `fetch_info`.
    let log_flavor = flavor.to_string();
    let outcome = runtime_fetch::ensure_fetched(
        &cache_dir,
        flavor,
        move |p| {
            tracing::info!(
                flavor = %log_flavor,
                phase = ?p.phase,
                "code_sandbox: fetch progress: {}",
                p.message
            );
        },
    )
    .await
    .map_err(|e| ReadyError::FetchFailed {
        flavor: flavor.to_string(),
        reason: fetch_error_message(&e),
    })?;
    let sqfs_path = outcome.installed_path.clone();
    let fetch_version = outcome.version.clone();
    let artifact_id = outcome.artifact_id;
    // Surface `fetch_info` only when this call actually downloaded.
    let fetch_info = if outcome.bytes_downloaded > 0 {
        Some(outcome)
    } else {
        None
    };

    // 1. Mount dir is parented at the per-version cache subdir so two
    //    versions of the same flavor never collide. Stem includes the
    //    full asset filename so a future swap can mount the new
    //    version alongside the draining old one.
    let stem = sqfs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sandbox-rootfs")
        .to_string();
    let mount_dir = cache_dir.join(&fetch_version).join(&stem);

    mount_if_needed(&sqfs_path, &mount_dir, flavor, &fetch_version).await?;

    // 2. Best-effort version sentinel read — informational only. The
    //    DB row is the source of truth for which release is mounted;
    //    a mismatch here is logged but no longer rejected.
    if let Ok(found) = read_version_sentinel(&mount_dir)
        && !found.is_empty()
        && found != fetch_version
    {
        tracing::warn!(
            mount = %mount_dir.display(),
            expected = %fetch_version,
            found = %found,
            "code_sandbox: rootfs version sentinel disagrees with DB row"
        );
    }

    // 3. PID-ns probe — needs a config view that points at THIS
    // flavor's mount dir, since probe_pid_ns invokes bwrap with
    // `--ro-bind <rootfs>/usr /usr`.
    let mut probe_cfg = state.config.clone();
    probe_cfg.rootfs_path = Some(mount_dir.to_string_lossy().into_owned());
    let caps = crate::modules::code_sandbox::probes::probe_rootfs_dependent(
        &probe_cfg,
        &state.host_caps,
    )
    .map_err(|reason| ReadyError::PidNsDisabled { reason })?;

    // Register the live mount with the version manager so a
    // subsequent pin-change can drain + evict against the right
    // inflight counter. Idempotent: a re-mount under the same
    // artifact_id reuses the existing `MountedArtifact`.
    let arch = std::env::consts::ARCH.to_string();
    crate::modules::code_sandbox::version_manager::register_mount(
        artifact_id,
        &fetch_version,
        &arch,
        flavor,
        mount_dir.clone(),
    );

    Ok(EnsureOutcome {
        caps: Arc::new(caps),
        mount_dir,
        fetch_info,
        artifact_id: Some(artifact_id),
        artifact_version: Some(fetch_version),
    })
}

/// Read the `.ziee-sandbox-rootfs-version` sentinel inside the mounted
/// rootfs. Best-effort: returns an empty string on dev builds (where
/// `build.sh --version` was unset) and a parse-friendly error on
/// missing/malformed sentinels. Soft because the DB row is now the
/// source of truth — a sentinel mismatch is just a breadcrumb for
/// debugging a misconfigured deployment.
fn read_version_sentinel(mount_dir: &Path) -> std::io::Result<String> {
    let p = mount_dir.join(".ziee-sandbox-rootfs-version");
    std::fs::read_to_string(p).map(|s| s.trim().to_string())
}

fn fetch_error_message(e: &FetchError) -> String {
    // FetchError implements Display; just stringify.
    e.to_string()
}

// Touch the FetchPhase import so the compiler doesn't warn about
// it being unused — the variant is referenced via debug-format only.
#[allow(dead_code)]
const _: Option<FetchPhase> = None;

// =====================================================================
// Cache dir + per-flavor squashfs lookup
// =====================================================================

fn derive_cache_dir(state: &CodeSandboxState) -> PathBuf {
    // The legacy `rootfs_path` config field points at a mounted tree
    // (e.g. `/var/lib/ziee/sandbox-rootfs/current`). Its parent is
    // the cache dir where per-flavor squashfs files live + where
    // per-flavor mount dirs sit.
    PathBuf::from(state.config.rootfs_path())
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Public accessor for the per-flavor cache dir (parent of the legacy
/// `current` mount symlink). Used by the streaming consent/fetch path
/// to drive `runtime_fetch::ensure_fetched` directly with progress.
pub fn cache_dir(state: &CodeSandboxState) -> PathBuf {
    derive_cache_dir(state)
}

/// `true` if a downloaded artifact for the currently-pinned version +
/// `flavor` already exists on disk, so picking that flavor will NOT
/// trigger a download. Used by the download-consent path in
/// `streaming.rs` to decide whether to prompt the user before
/// auto-fetching multi-hundred-MB rootfs payloads.
///
/// Conservative on errors: any DB / FS failure resolves to "not
/// cached", so the consent prompt fires (over-prompting is far less
/// bad than silently downloading without consent).
pub async fn is_flavor_cached(state: &CodeSandboxState, flavor: &str) -> bool {
    use crate::modules::code_sandbox::version_manager;
    let pool = match state.pool.as_ref() {
        Some(p) => p,
        None => return false,
    };
    let pinned = match version_manager::current_pin(pool).await {
        Ok(Some(v)) => v,
        _ => return false,
    };
    let arch = std::env::consts::ARCH;
    let package = if cfg!(target_os = "windows") {
        "tar.zst"
    } else {
        "squashfs"
    };
    match version_manager::find_artifact(pool, &pinned, arch, flavor, package).await {
        Ok(Some(row)) => std::path::PathBuf::from(&row.artifact_path).exists(),
        _ => false,
    }
}

// =====================================================================
// squashfuse spawn (foreground + PDEATHSIG)
// =====================================================================

/// Compose the MOUNTED registry key. Two pinned versions of the same
/// flavor must coexist in the registry during a swap-drain cycle (the
/// old version's squashfuse stays alive until inflight==0 even though
/// the new pin is already serving fresh `execute_command`s), so the
/// key encodes BOTH coordinates. Plan 5 Phase 2 explicitly: "`MOUNTED`
/// registry key flips from `flavor` to `(version, arch, flavor)`."
fn mount_key(version: &str, flavor: &str) -> String {
    if version.is_empty() {
        flavor.to_string()
    } else {
        format!("{version}/{flavor}")
    }
}

async fn mount_if_needed(
    sqfs_path: &Path,
    mount_dir: &Path,
    flavor: &str,
    version: &str,
) -> Result<(), ReadyError> {
    // Idempotency: if the mount dir already has `usr/`, it's mounted
    // (by the test harness, an operator pre-mount via `just
    // sandbox-mount`, or a stale prior-server mount). Skip the spawn
    // and don't take ownership — shutdown() only tears down what
    // this process spawned.
    if mount_dir.join("usr").exists() {
        tracing::info!(
            mount_dir = %mount_dir.display(),
            flavor,
            "code_sandbox: rootfs already mounted; skipping squashfuse spawn"
        );
        return Ok(());
    }

    if let Err(e) = std::fs::create_dir_all(mount_dir) {
        return Err(ReadyError::MountFailed {
            reason: format!("mkdir {}: {e}", mount_dir.display()),
        });
    }

    tracing::info!(
        sqfs = %sqfs_path.display(),
        mount = %mount_dir.display(),
        flavor,
        "code_sandbox: lazy-init spawning squashfuse"
    );

    // Foreground squashfuse + PDEATHSIG ensures the FUSE daemon dies
    // with the server even on SIGKILL/OOM-kill.
    let mut cmd = Command::new("squashfuse");
    cmd.arg("-f").arg(sqfs_path).arg(mount_dir);
    // PR_SET_PDEATHSIG makes the FUSE daemon die with the server even on
    // SIGKILL/OOM. Linux-only; macOS/Windows mount inside the VM/WSL2 guest.
    #[cfg(target_os = "linux")]
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);
            if r == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadyError::SquashfuseMissing);
        }
        Err(e) => {
            return Err(ReadyError::MountFailed {
                reason: format!("squashfuse spawn: {e}"),
            });
        }
    };

    // Stash the Child in MOUNTED so shutdown() can clean it up.
    // Dropping a tokio::process::Child does NOT kill the process.
    let slot = MOUNTED
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;
    {
        let mut g = slot.lock().await;
        g.insert(
            mount_key(version, flavor),
            MountedRootfs {
                child,
                mount_dir: mount_dir.to_path_buf(),
            },
        );
    }

    // Wait for the mount table to update (squashfuse forks quickly
    // but the kernel mount visibility is async). 5 s ceiling — well
    // above the typical ~50-100 ms.
    let target_usr = mount_dir.join("usr");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !target_usr.exists() {
        if Instant::now() > deadline {
            return Err(ReadyError::MountFailed {
                reason: format!(
                    "squashfuse spawned but {}/usr did not appear within 5 s",
                    mount_dir.display()
                ),
            });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Ok(())
}

// =====================================================================
// Shutdown: tear down all spawned squashfuse children
// =====================================================================

/// Iterate every flavor's spawned squashfuse Child and stop it
/// politely (SIGTERM + 2 s wait + defensive `fusermount -u`).
/// Called from `main.rs::shutdown_signal`. Idempotent.
///
/// For force-quit paths (SIGKILL of the server), this never runs;
/// every squashfuse instance was spawned with PDEATHSIG=SIGTERM, so
/// the kernel does the cleanup.
pub async fn shutdown() {
    let Some(slot) = MOUNTED.get() else { return };
    let mut guard = slot.lock().await;
    let mounts: Vec<(String, MountedRootfs)> = guard.drain().collect();
    drop(guard); // release lock; we no longer need it

    for (flavor, mounted) in mounts {
        let MountedRootfs { mut child, mount_dir } = mounted;

        // SIGTERM first — squashfuse's default handler unmounts
        // cleanly. (tokio's Child::kill sends SIGKILL, which would
        // leave the mount table referencing a dead process.)
        if let Some(pid) = child.id() {
            #[cfg(target_os = "linux")]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            #[cfg(not(target_os = "linux"))]
            let _ = pid;
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        }
        // Defensive unmount in case squashfuse died without
        // cleaning up its own mount.
        let _ = Command::new("fusermount")
            .arg("-u")
            .arg(&mount_dir)
            .status()
            .await;
        tracing::info!(
            mount = %mount_dir.display(),
            flavor,
            "code_sandbox: rootfs unmounted on shutdown"
        );
    }
}

/// Snapshot of flavors currently mounted (server-spawned squashfuse live).
/// Empty if no flavor has been mounted yet. Projects the (version,
/// flavor) composite MOUNTED keys back to a flavor-only set for the
/// legacy `/code-sandbox/environments` endpoint.
pub async fn mounted_set() -> std::collections::HashSet<String> {
    match MOUNTED.get() {
        Some(slot) => slot
            .lock()
            .await
            .keys()
            .map(|k| {
                // Composite key shape "<version>/<flavor>" — split off the
                // flavor; fall back to the whole key for legacy entries
                // (pre-version-aware mounts).
                k.rsplit_once('/').map(|(_, f)| f.to_string()).unwrap_or_else(|| k.clone())
            })
            .collect(),
        None => std::collections::HashSet::new(),
    }
}

/// Result of evicting a flavor from the cache.
#[derive(Debug, Clone, Copy)]
pub struct EvictOutcome {
    pub bytes_freed: u64,
    pub was_cached: bool,
}

/// Evict a flavor from the cache: unmount EVERY pinned version's
/// squashfuse process for this flavor (if mounted), drop their
/// READY/MOUNTED registry entries, and delete all cached
/// `*-{flavor}.squashfs` files under `cache_dir`. Used by the legacy
/// `/code-sandbox/environments/{flavor}` admin DELETE endpoint.
/// Idempotent.
///
/// For version-aware eviction (the Plan 5 Phase 3 drain-on-swap
/// path), use [`evict_by_version_flavor`] instead — it kills only
/// the specific `(version, flavor)` mount and leaves any sibling
/// version of the same flavor running.
pub async fn evict_flavor(cache_dir: &Path, flavor: &str) -> EvictOutcome {
    // 1. Drop READY cells for any (version, flavor) entry matching
    //    this flavor.
    if let Some(map) = READY.get() {
        let suffix = format!("/{flavor}");
        map.retain(|k, _| k != flavor && !k.ends_with(&suffix));
    }

    // 2. Unmount every squashfuse spawn for this flavor (across all
    //    pinned versions). Same defensive sequence as shutdown().
    if let Some(slot) = MOUNTED.get() {
        let suffix = format!("/{flavor}");
        let stale_keys: Vec<String> = slot
            .lock()
            .await
            .keys()
            .filter(|k| k.as_str() == flavor || k.ends_with(&suffix))
            .cloned()
            .collect();
        for key in stale_keys {
            let taken = slot.lock().await.remove(&key);
            if let Some(MountedRootfs { mut child, mount_dir }) = taken {
                if let Some(pid) = child.id() {
                    #[cfg(target_os = "linux")]
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                    #[cfg(not(target_os = "linux"))]
                    let _ = pid;
                    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                }
                let _ = Command::new("fusermount").arg("-u").arg(&mount_dir).status().await;
                let _ = std::fs::remove_dir(&mount_dir);
            }
        }
    }

    // 3. Delete the cached squashfs file(s) for this flavor + mirror mount dir.
    let suffix = format!("-{flavor}.squashfs");
    let mut bytes_freed: u64 = 0;
    let mut was_cached = false;
    if let Ok(rd) = std::fs::read_dir(cache_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            let is_match = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix));
            if !is_match {
                continue;
            }
            was_cached = true;
            if let Ok(meta) = std::fs::metadata(&p) {
                bytes_freed += meta.len();
            }
            // Best-effort unmount + remove the mirror mount dir (the squashfs
            // basename without its extension).
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                let mnt = cache_dir.join(stem);
                let _ = Command::new("fusermount").arg("-u").arg(&mnt).status().await;
                let _ = std::fs::remove_dir_all(&mnt);
            }
            match std::fs::remove_file(&p) {
                Ok(()) => tracing::info!(
                    path = %p.display(), flavor, "code_sandbox: rootfs evicted from cache"
                ),
                Err(e) => tracing::warn!(
                    path = %p.display(), error = %e,
                    "code_sandbox: evict failed to remove squashfs"
                ),
            }
        }
    }

    EvictOutcome { bytes_freed, was_cached }
}

/// Version-aware evict: tear down ONLY the `(version, flavor)`
/// mount, leaving any sibling version of the same flavor (or any
/// other flavor) untouched. Plan 5 Phase 3 drain-on-swap path.
///
/// `version_cache_dir` is the per-version cache subdir
/// (`<rootfs cache root>/<version>/`) so the cached squashfs at
/// that path is removed too.
pub async fn evict_by_version_flavor(
    version_cache_dir: &Path,
    version: &str,
    flavor: &str,
) -> EvictOutcome {
    let key = mount_key(version, flavor);

    // 1. Drop the matching READY cell so the next call re-mounts.
    if let Some(map) = READY.get() {
        map.remove(&key);
    }

    // 2. Unmount the specific squashfuse for this (version, flavor).
    if let Some(slot) = MOUNTED.get() {
        let taken = slot.lock().await.remove(&key);
        if let Some(MountedRootfs { mut child, mount_dir }) = taken {
            if let Some(pid) = child.id() {
                #[cfg(target_os = "linux")]
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
                #[cfg(not(target_os = "linux"))]
                let _ = pid;
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            }
            let _ = Command::new("fusermount").arg("-u").arg(&mount_dir).status().await;
            let _ = std::fs::remove_dir(&mount_dir);
        }
    }

    // 3. Remove the per-version cached squashfs for THIS flavor.
    let suffix = format!("-{flavor}.squashfs");
    let mut bytes_freed: u64 = 0;
    let mut was_cached = false;
    if let Ok(rd) = std::fs::read_dir(version_cache_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            let is_match = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix));
            if !is_match {
                continue;
            }
            was_cached = true;
            if let Ok(meta) = std::fs::metadata(&p) {
                bytes_freed += meta.len();
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                let mnt = version_cache_dir.join(stem);
                let _ = Command::new("fusermount").arg("-u").arg(&mnt).status().await;
                let _ = std::fs::remove_dir_all(&mnt);
            }
            let _ = std::fs::remove_file(&p);
        }
    }
    tracing::info!(
        version,
        flavor,
        bytes_freed,
        was_cached,
        "code_sandbox: evict_by_version_flavor complete"
    );
    EvictOutcome { bytes_freed, was_cached }
}
