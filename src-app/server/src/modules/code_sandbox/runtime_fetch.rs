//! Backward-compatible thin shim over the `version_manager`.
//!
//! Until Plan 5 Phase 2b, this module owned the entire fetch
//! pipeline: TOML-based resolver, GitHub Releases URL builder, sha256
//! check, cosign verifier. That logic is now in `version_manager`,
//! where it's driven by the DB pin instead of an embedded
//! `known_revisions.toml`.
//!
//! What survives here:
//!   * `FetchOutcome` + `FetchProgress` + `FetchPhase` — the public
//!     shapes consumed by the streaming / admin-install / backend code,
//!     kept stable so call sites didn't have to change.
//!   * `fetch_flavor` / `fetch_flavor_format` / `ensure_fetched` /
//!     `ensure_fetched_format` — the public entry points, now thin
//!     adapters around `version_manager::install_version`.
//!   * `is_fetch_in_flight` — preserves the in-flight-probe API used
//!     by the download-consent path in `streaming.rs`.
//!   * `download_blob_blocking` + `verify_cosign_blob` — `pub(crate)`
//!     low-level primitives shared with `version_manager` so the
//!     reqwest::blocking + sigstore plumbing has one home.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

// =====================================================================
// Public surface (preserved from the prior module)
// =====================================================================

#[derive(Debug, Clone)]
pub struct FetchProgress {
    pub phase: FetchPhase,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FetchPhase {
    Resolving,
    Downloading,
    VerifyingSha256,
    VerifyingCosign,
    Installing,
}

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    pub installed_path: PathBuf,
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
    pub cosign_verified: bool,
    /// Semver string identifying which rootfs release this artifact
    /// belongs to. Surfaced via `fetch_info.version` in the chat UI.
    pub version: String,
    /// PK of the `code_sandbox_rootfs_artifacts` row corresponding to
    /// this fetch. Plumbed through so `runtime_mount` can register the
    /// mount + every caller can `version_manager::acquire_inflight`
    /// for the drain-on-swap protocol (Plan 5 Phase 3).
    pub artifact_id: uuid::Uuid,
}

/// Packaging variant. The squashfs is the universal artifact
/// (Linux squashfuse + macOS in-guest mount); the `.tar.zst` tarball
/// exists only for Windows `wsl --import` (which can't consume a
/// squashfs). Both are produced from the identical staged tree at
/// release time, so both share the rootfs content but ship in different
/// container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootfsFormat {
    Squashfs,
    TarZst,
}

impl RootfsFormat {
    /// File-name extension (no leading dot) for this packaging.
    pub fn ext(self) -> &'static str {
        match self {
            RootfsFormat::Squashfs => "squashfs",
            RootfsFormat::TarZst => "tar.zst",
        }
    }
}

#[derive(Debug, Clone)]
pub enum FetchError {
    /// Stable catch-all surfaced from the version_manager — the inner
    /// message carries the structured error code
    /// (`SANDBOX_ROOTFS_UNAVAILABLE`, `SANDBOX_ROOTFS_VERSION_MISSING`,
    /// …). Other variants were retired with the prior TOML resolver.
    Install(String),
    /// Download failed for network reasons.
    Download(String),
    /// sha256 sidecar disagreed with the downloaded artifact.
    Sha256Mismatch { expected: String, got: String },
    /// cosign keyless verification failed.
    CosignFailed(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Install(e) => write!(f, "{e}"),
            FetchError::Download(e) => write!(f, "download failed: {e}"),
            FetchError::Sha256Mismatch { expected, got } => {
                write!(f, "sha256 mismatch (expected {expected}, got {got})")
            }
            FetchError::CosignFailed(e) => write!(f, "cosign verification failed: {e}"),
        }
    }
}

// =====================================================================
// Single-flight per-flavor fetch lock (used by both auto-fetch and the
// admin install path). Lives here for backwards compatibility with
// the streaming.rs in-flight probe; the underlying download is
// serialized again inside `version_manager` on its richer
// `(version, arch, flavor, package)` key.
// =====================================================================

static FETCH_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

fn fetch_lock_for(flavor: &str) -> Arc<Mutex<()>> {
    FETCH_LOCKS
        .entry(flavor.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// `true` if a download for `flavor` is currently in flight. Used by
/// the download-consent path so we don't re-prompt mid-download.
pub fn is_fetch_in_flight(flavor: &str) -> bool {
    FETCH_LOCKS
        .get(flavor)
        .map(|m| m.value().try_lock().is_err())
        .unwrap_or(false)
}

// =====================================================================
// Public fetch entry points — adapters over version_manager
// =====================================================================

/// Resolve, download, verify, and install the squashfs for `flavor`
/// matching the system-wide pinned rootfs version. Idempotent.
pub async fn fetch_flavor(
    cache_dir: &Path,
    flavor: &str,
    progress: impl Fn(FetchProgress) + Send + Sync + 'static,
) -> Result<FetchOutcome, FetchError> {
    fetch_flavor_format(cache_dir, flavor, RootfsFormat::Squashfs, progress).await
}

/// Like [`fetch_flavor`] but for a specific packaging. The Windows WSL2
/// backend fetches [`RootfsFormat::TarZst`]; everything else uses the
/// squashfs default.
pub async fn fetch_flavor_format(
    cache_dir: &Path,
    flavor: &str,
    format: RootfsFormat,
    progress: impl Fn(FetchProgress) + Send + Sync + 'static,
) -> Result<FetchOutcome, FetchError> {
    use crate::modules::code_sandbox::version_manager;

    let state = crate::modules::code_sandbox::config::get_state().ok_or_else(|| {
        FetchError::Install(
            "code_sandbox not initialized; cannot resolve rootfs artifact".to_string(),
        )
    })?;
    let pool = state.pool.as_ref().ok_or_else(|| {
        FetchError::Install(
            "code_sandbox state is missing the DB pool; cannot resolve rootfs artifact"
                .to_string(),
        )
    })?;

    let pkg = match format {
        RootfsFormat::Squashfs => "squashfs",
        RootfsFormat::TarZst => "tar.zst",
    };
    let arch = std::env::consts::ARCH;

    // Resolve the pinned version (lazy-init the pin if it's NULL).
    let pinned = match version_manager::ensure_pin_initialized(pool).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Err(FetchError::Install(
                "no rootfs version is pinned and GitHub is unreachable; \
                 an admin must set a pin manually"
                    .to_string(),
            ));
        }
        Err(e) => return Err(FetchError::Install(format!("pin resolution failed: {e}"))),
    };

    // Bridge progress events from the version manager's InstallProgress
    // back into the FetchProgress shape used by callers' SSE
    // streams.
    let progress = Arc::new(progress);
    let progress_clone = progress.clone();
    let install_progress = move |ev: version_manager::InstallProgress| {
        let (phase, message) = match ev {
            version_manager::InstallProgress::Resolving { version, asset } => (
                FetchPhase::Resolving,
                format!("resolving v{version} {asset}"),
            ),
            version_manager::InstallProgress::Downloading { url } => {
                (FetchPhase::Downloading, format!("downloading {url}"))
            }
            version_manager::InstallProgress::VerifyingSha256 => {
                (FetchPhase::VerifyingSha256, "verifying sha256".to_string())
            }
            version_manager::InstallProgress::VerifyingCosign => (
                FetchPhase::VerifyingCosign,
                "verifying cosign signature".to_string(),
            ),
            version_manager::InstallProgress::Installing { path } => {
                (FetchPhase::Installing, format!("installing {path}"))
            }
        };
        progress_clone(FetchProgress { phase, message });
    };

    let (row, stats) = version_manager::install_version(
        pool,
        cache_dir,
        &pinned,
        arch,
        flavor,
        pkg,
        install_progress,
    )
    .await
    .map_err(|e| {
        let msg = e.to_string();
        match e {
            version_manager::VersionError::Sha256Mismatch { expected, got } => {
                FetchError::Sha256Mismatch { expected, got }
            }
            version_manager::VersionError::CosignFailed(s) => FetchError::CosignFailed(s),
            version_manager::VersionError::GitHubUnreachable(_)
            | version_manager::VersionError::Io(_) => FetchError::Download(msg),
            _ => FetchError::Install(msg),
        }
    })?;

    // Touch last_used_at best-effort so the admin UI can sort by recency.
    version_manager::touch_last_used(pool, row.id).await;
    let _ = progress; // keep the Arc alive across the install call

    let (bytes_downloaded, duration_ms, cosign_verified) = match stats {
        Some(s) => (s.bytes_downloaded, s.duration_ms, s.cosign_verified),
        None => (0, 0, row.cosign_bundle.is_some()),
    };

    Ok(FetchOutcome {
        installed_path: PathBuf::from(row.artifact_path),
        bytes_downloaded,
        duration_ms,
        cosign_verified,
        version: row.version,
        artifact_id: row.id,
    })
}

/// Single-flight wrapper: serializes per-flavor downloads so the
/// admin "Install" SSE flow + in-conversation auto-fetch never collide.
pub async fn ensure_fetched(
    cache_dir: &Path,
    flavor: &str,
    progress: impl Fn(FetchProgress) + Send + Sync + 'static,
) -> Result<FetchOutcome, FetchError> {
    ensure_fetched_format(cache_dir, flavor, RootfsFormat::Squashfs, progress).await
}

pub async fn ensure_fetched_format(
    cache_dir: &Path,
    flavor: &str,
    format: RootfsFormat,
    progress: impl Fn(FetchProgress) + Send + Sync + 'static,
) -> Result<FetchOutcome, FetchError> {
    let lock = fetch_lock_for(flavor);
    let _guard = lock.lock().await;
    fetch_flavor_format(cache_dir, flavor, format, progress).await
}

// =====================================================================
// Crate-public primitives reused by `version_manager`
// =====================================================================

/// Download `url` into `dest`, retrying up to `attempts` times.
/// Returns the byte count on success, or a stringified error message
/// on failure (including 404, network errors, and write errors).
/// Blocking — must be called from a `tokio::spawn_blocking` context.
pub(crate) fn download_blob_blocking(
    url: &str,
    dest: &Path,
    attempts: u32,
) -> Result<u64, String> {
    match download_to_file(url, dest, attempts) {
        DownloadResult::Ok(n) => Ok(n),
        DownloadResult::NotFound => Err(format!("HTTP 404 at {url}")),
        DownloadResult::Failed(e) => Err(e),
    }
}

/// Crate-public wrapper around the in-house cosign verifier so
/// `version_manager` can reuse the same trust root + identity policy
/// without re-implementing sigstore plumbing.
pub(crate) fn verify_cosign_blob(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    verify_cosign_bundle(bundle_path, blob_path, identity, issuer)
}

// =====================================================================
// Download (reqwest::blocking — runs on this thread, no nested runtime)
// =====================================================================

enum DownloadResult {
    Ok(u64),
    NotFound,
    Failed(String),
}

fn download_to_file(url: &str, dest: &Path, attempts: u32) -> DownloadResult {
    use std::io::{Read, Write};

    // Hard size cap. Full rootfs squashfs images run ~1.6-2.0 GB, so 4 GiB
    // leaves generous headroom while still bounding the bytes an
    // attacker-controlled / hijacked-mirror `/dev/zero` stream can spool to
    // disk before sha256 verification rejects it. Parity with
    // `llm_local_runtime::engine::download`'s `MAX_DOWNLOAD_BYTES` (which uses
    // a tighter 2 GiB cap suited to engine binaries).
    const MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

    let builder = reqwest::blocking::Client::builder().timeout(Duration::from_secs(600));
    // Release builds only ever download from `https://github.com/...`; reject
    // plaintext transports there. The debug-only loopback mirror
    // (`CODE_SANDBOX_ROOTFS_MIRROR`, http) needs the relaxed client. Shadow
    // (not `mut`) so debug builds don't warn about an unused `mut`.
    #[cfg(not(debug_assertions))]
    let builder = builder.https_only(true);
    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => return DownloadResult::Failed(format!("client build: {e}")),
    };

    let mut last_err = String::new();
    for attempt in 1..=attempts {
        match client.get(url).send() {
            Ok(resp) => {
                let status = resp.status();
                if status == reqwest::StatusCode::NOT_FOUND {
                    return DownloadResult::NotFound;
                }
                if !status.is_success() {
                    last_err = format!("HTTP {status}");
                    if status.is_server_error() && attempt < attempts {
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    return DownloadResult::Failed(last_err);
                }
                // Content-Length pre-check: fail fast before writing a byte.
                if let Some(len) = resp.content_length()
                    && len > MAX_DOWNLOAD_BYTES
                {
                    return DownloadResult::Failed(format!(
                        "refusing to download {len} bytes (cap {MAX_DOWNLOAD_BYTES} / 4 GiB)"
                    ));
                }
                let mut file = match std::fs::File::create(dest) {
                    Ok(f) => f,
                    Err(e) => {
                        return DownloadResult::Failed(format!(
                            "create {}: {e}",
                            dest.display()
                        ))
                    }
                };
                // Chunked copy with a running byte cap — a server that lies
                // about (or omits) Content-Length is still bounded mid-stream.
                let mut resp = resp;
                let mut received: u64 = 0;
                let mut buf = [0u8; 64 * 1024];
                let mut stream_err: Option<String> = None;
                let mut capped = false;
                loop {
                    match resp.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            received = received.saturating_add(n as u64);
                            if received > MAX_DOWNLOAD_BYTES {
                                capped = true;
                                break;
                            }
                            if let Err(e) = file.write_all(&buf[..n]) {
                                stream_err = Some(format!("write {}: {e}", dest.display()));
                                break;
                            }
                        }
                        Err(e) => {
                            stream_err = Some(format!("stream-read: {e}"));
                            break;
                        }
                    }
                }
                if capped {
                    let _ = std::fs::remove_file(dest);
                    // Not transient — don't retry a body that overflowed the cap.
                    return DownloadResult::Failed(format!(
                        "download exceeded {MAX_DOWNLOAD_BYTES} bytes / 4 GiB cap; aborted"
                    ));
                }
                if let Some(e) = stream_err {
                    last_err = e;
                    let _ = std::fs::remove_file(dest);
                    if attempt < attempts {
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    return DownloadResult::Failed(last_err);
                }
                return DownloadResult::Ok(received);
            }
            Err(e) => {
                last_err = format!("send: {e}");
                if attempt < attempts {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                return DownloadResult::Failed(last_err);
            }
        }
    }
    DownloadResult::Failed(last_err)
}

// =====================================================================
// Cosign keyless OIDC verification (sigstore crate)
// =====================================================================

fn verify_cosign_bundle(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    use sigstore::bundle::verify::blocking::Verifier;
    use sigstore::bundle::verify::policy::Identity;
    use sigstore::bundle::Bundle;

    let bundle_json =
        std::fs::read_to_string(bundle_path).map_err(|e| format!("read bundle: {e}"))?;
    let bundle: Bundle =
        serde_json::from_str(&bundle_json).map_err(|e| format!("parse bundle: {e}"))?;
    let blob = std::fs::File::open(blob_path).map_err(|e| format!("open blob: {e}"))?;
    let verifier = Verifier::production().map_err(|e| format!("trust root init: {e}"))?;
    let policy = Identity::new(identity, issuer);
    verifier
        .verify(blob, bundle, &policy, false)
        .map_err(|e| format!("signature verification: {e}"))?;
    Ok(())
}

// =====================================================================
// Tier 1 — unit tests for the surviving single-flight lock helpers
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn fetch_lock_is_per_flavor_and_stable() {
        let a1 = fetch_lock_for("alpha");
        let a2 = fetch_lock_for("alpha");
        let b = fetch_lock_for("beta");
        assert!(Arc::ptr_eq(&a1, &a2));
        assert!(!Arc::ptr_eq(&a1, &b));
    }

    #[tokio::test]
    async fn is_fetch_in_flight_reflects_lock_state() {
        let flavor = "in-flight-probe";
        assert!(!is_fetch_in_flight(flavor));
        let lock = fetch_lock_for(flavor);
        let guard = lock.lock().await;
        assert!(is_fetch_in_flight(flavor));
        drop(guard);
        assert!(!is_fetch_in_flight(flavor));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn same_flavor_serializes_distinct_flavors_parallel() {
        let max_same = Arc::new(AtomicUsize::new(0));
        let cur_same = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let max_same = max_same.clone();
            let cur_same = cur_same.clone();
            handles.push(tokio::spawn(async move {
                let lock = fetch_lock_for("serialize-me");
                let _g = lock.lock().await;
                let now = cur_same.fetch_add(1, Ordering::SeqCst) + 1;
                max_same.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(10)).await;
                cur_same.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(max_same.load(Ordering::SeqCst), 1);

        let la = fetch_lock_for("flav-a");
        let lb = fetch_lock_for("flav-b");
        let _ga = la.lock().await;
        assert!(lb.try_lock().is_ok());
    }
}
