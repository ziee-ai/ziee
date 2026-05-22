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
//!   - Run `probe_rootfs_schema` + `probe_pid_ns` against the
//!     flavor-specific mount. Cache the resulting
//!     `HardeningCapabilities` in [`READY[flavor]`].
//! - **Subsequent calls for the same flavor**: atomic load from the
//!   per-flavor `OnceCell`. Cached failure is sticky.
//! - **Subsequent calls for a different flavor**: spawn squashfuse
//!   for THAT flavor, mount alongside. Both stay live.
//! - **Server shutdown** ([`shutdown`]): iterate the MOUNTED map,
//!   kill every Child + `fusermount -u` each mount dir.
//! - **Force-quit (SIGKILL)**: PDEATHSIG delivers SIGTERM to every
//!   squashfuse child. Each unmounts itself. No app cooperation.

use std::collections::HashMap;
use std::os::unix::process::CommandExt;
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
use crate::modules::code_sandbox::{probe_rootfs_schema, SANDBOX_ROOTFS_SCHEMA_VERSION};

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
#[derive(Debug, Clone)]
pub struct EnsureOutcome {
    pub caps: Arc<HardeningCapabilities>,
    pub mount_dir: PathBuf,
    pub fetch_info: Option<FetchOutcome>,
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
    SchemaMismatch { found: u32, expected: u32 },
    SchemaReadFailed { reason: String },
    PidNsDisabled { reason: String },
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
                    "sandbox cannot start: no rootfs squashfs for flavor {flavor:?} in {}. \
                     This is normally auto-fetched on first use; the failure indicates \
                     either no network at startup or the flavor isn't in the embedded \
                     known_revisions.toml.",
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
            ReadyError::SchemaMismatch { found, expected } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_SCHEMA_MISMATCH",
                format!(
                    "sandbox cannot start: rootfs schema v{found} but this server \
                     binary expects v{expected}. Upgrade the rootfs."
                ),
            ),
            ReadyError::SchemaReadFailed { reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_SCHEMA_UNREADABLE",
                format!(
                    "sandbox cannot start: rootfs schema sentinel unreadable: \
                     {reason}. The rootfs may be corrupt."
                ),
            ),
            ReadyError::PidNsDisabled { reason } => AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_PIDNS_DISABLED",
                format!("sandbox cannot start: {reason}"),
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
    let ready_map = READY.get_or_init(|| async { DashMap::new() }).await;
    let cell: ReadyCell = ready_map
        .entry(flavor.to_string())
        .or_insert_with(|| Arc::new(OnceCell::new()))
        .clone();
    let cached = cell
        .get_or_init(|| async { do_first_init(state, flavor).await })
        .await;
    match cached {
        Ok(outcome) => Ok(outcome.clone()),
        Err(e) => Err(e.to_app_error()),
    }
}

async fn do_first_init(state: &CodeSandboxState, flavor: &str) -> ReadyResult {
    let cache_dir = derive_cache_dir(state);

    // 0. Auto-fetch on miss. If the cache dir already has a
    // .squashfs for this flavor, we skip the network entirely
    // (idempotent — `fetch_flavor` short-circuits on cache hit
    // too, but skipping it here avoids the tokio::spawn_blocking
    // overhead for the common warm path).
    let (sqfs_path, fetch_info) =
        match find_cached_squashfs_for_flavor(&cache_dir, flavor) {
            Some(p) => (p, None),
            None => {
                tracing::info!(
                    flavor,
                    cache_dir = %cache_dir.display(),
                    "code_sandbox: no cached rootfs for flavor; auto-fetching"
                );
                let log_flavor = flavor.to_string();
                let outcome = runtime_fetch::fetch_flavor(
                    &cache_dir,
                    flavor,
                    move |p| {
                        // Use tracing for now; Phase 4's structured
                        // fetch_info field is the user-facing surface.
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
                (outcome.installed_path.clone(), Some(outcome))
            }
        };

    let stem = sqfs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sandbox-rootfs")
        .to_string();
    let mount_dir = cache_dir.join(&stem);

    // 1. Mount the squashfs (skip if already mounted by harness/prior).
    mount_if_needed(&sqfs_path, &mount_dir, flavor).await?;

    // 2. Schema sentinel check against the flavor's mount.
    let schema = probe_rootfs_schema(mount_dir.to_str().unwrap_or_default())
        .map_err(|reason| ReadyError::SchemaReadFailed { reason })?;
    if schema != SANDBOX_ROOTFS_SCHEMA_VERSION {
        return Err(ReadyError::SchemaMismatch {
            found: schema,
            expected: SANDBOX_ROOTFS_SCHEMA_VERSION,
        });
    }

    // 3. PID-ns probe — needs a config view that points at THIS
    // flavor's mount dir, since probe_pid_ns invokes bwrap with
    // `--ro-bind <rootfs>/usr /usr`.
    let mut probe_cfg = state.config.clone();
    probe_cfg.rootfs_path = mount_dir.to_string_lossy().into_owned();
    let caps = crate::modules::code_sandbox::probes::probe_rootfs_dependent(
        &probe_cfg,
        &state.host_caps,
    )
    .map_err(|reason| ReadyError::PidNsDisabled { reason })?;

    Ok(EnsureOutcome {
        caps: Arc::new(caps),
        mount_dir,
        fetch_info,
    })
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
    PathBuf::from(&state.config.rootfs_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Find the most-recently-modified `.squashfs` in `cache_dir` whose
/// filename ends with `-{flavor}.squashfs`. Returns `None` if no
/// matching file exists.
fn find_cached_squashfs_for_flavor(cache_dir: &Path, flavor: &str) -> Option<PathBuf> {
    let suffix = format!("-{flavor}.squashfs");
    let mut matches: Vec<PathBuf> = std::fs::read_dir(cache_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix))
        })
        .collect();
    matches.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    matches.last().cloned()
}

// =====================================================================
// squashfuse spawn (foreground + PDEATHSIG)
// =====================================================================

async fn mount_if_needed(
    sqfs_path: &Path,
    mount_dir: &Path,
    flavor: &str,
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
            flavor.to_string(),
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
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
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
/// Empty if no flavor has been mounted yet.
pub async fn mounted_set() -> std::collections::HashSet<String> {
    match MOUNTED.get() {
        Some(slot) => slot.lock().await.keys().cloned().collect(),
        None => std::collections::HashSet::new(),
    }
}

/// Result of evicting a flavor from the cache.
#[derive(Debug, Clone, Copy)]
pub struct EvictOutcome {
    pub bytes_freed: u64,
    pub was_cached: bool,
}

/// Evict a flavor from the cache: unmount its squashfuse (if mounted),
/// drop its READY/MOUNTED registry entries, and delete its cached
/// `*-{flavor}.squashfs` file(s) under `cache_dir` plus any mirror mount dir.
/// Idempotent — returns `was_cached: false`, `bytes_freed: 0` when nothing is
/// present. The next `ensure_rootfs_ready` for the flavor re-fetches + re-mounts.
pub async fn evict_flavor(cache_dir: &Path, flavor: &str) -> EvictOutcome {
    // 1. Drop the READY cell so the next use re-initializes from scratch.
    if let Some(map) = READY.get() {
        map.remove(flavor);
    }

    // 2. Unmount the squashfuse we spawned (if any) — mirrors shutdown().
    if let Some(slot) = MOUNTED.get() {
        let taken = slot.lock().await.remove(flavor);
        if let Some(MountedRootfs { mut child, mount_dir }) = taken {
            if let Some(pid) = child.id() {
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            }
            let _ = Command::new("fusermount").arg("-u").arg(&mount_dir).status().await;
            let _ = std::fs::remove_dir(&mount_dir);
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
