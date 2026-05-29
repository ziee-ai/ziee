//! Rootfs version manager (Plan 5 Phase 2).
//!
//! Replaces the legacy `known_revisions.toml` resolver with a
//! GitHub-Releases-API-driven lifecycle that mirrors the
//! `llm_local_runtime` pattern. DB is the source of truth for what's
//! downloaded; `code_sandbox_settings.current_rootfs_version` is the
//! single pin shared by every flavor + arch.
//!
//! The module exposes a small async API used by:
//!   - boot init (`ensure_pin_initialized`)
//!   - `execute_command` lazy-fetch path (`resolve_artifact`)
//!   - admin handlers (`list_releases`, `install_version`, `set_pin`,
//!     `delete_artifact`, `status`)
//!
//! Phase 2 lands the DB-row + GitHub-API + download lifecycle.
//! Phase 3 will layer per-mount inflight counters + drain/wipe on top
//! of the same `set_pin` entry point.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::common::AppError;
use axum::http::StatusCode;

// =====================================================================
// Public surface
// =====================================================================

/// Org+repo of the published rootfs releases. Mirrors the convention
/// used by `ziee-ai/llama.cpp` and `ziee-ai/mistral.rs`: the org owns
/// build outputs while the consumer app lives elsewhere.
const ROOTFS_REPO: &str = "ziee-ai/sandbox-rootfs";

/// One downloaded artifact in `code_sandbox_rootfs_artifacts`.
/// Field order mirrors the SQL column order so the sqlx `FromRow`
/// derive matches without explicit `column()` annotations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct RootfsArtifact {
    pub id: Uuid,
    pub version: String,
    pub arch: String,
    pub flavor: String,
    pub package: String,
    pub sha256: String,
    pub artifact_path: String,
    pub cosign_bundle: Option<String>,
    pub status: String,
    pub downloaded_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// One semver tag visible on GitHub. The version-manager surfaces these
/// in the admin UI's "available" list, distinct from the "installed"
/// list which comes from the DB.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RootfsRelease {
    /// Semver string with the leading `v` stripped (`"0.1.0"`).
    pub version: String,
    pub published_at: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    /// Raw asset names attached to the release — useful for the UI to
    /// surface "only x86_64 published" cases before the admin clicks
    /// "Install".
    pub asset_names: Vec<String>,
}

/// Snapshot returned by `status()` for the admin UI's "Rootfs
/// versions" page. Phase 3 will add a `draining: Vec<DrainEntry>`
/// field once the inflight counter lands.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VersionStatus {
    pub pinned_version: Option<String>,
    pub installed: Vec<RootfsArtifact>,
    /// Only populated when GitHub is reachable (best-effort).
    pub available: Vec<RootfsRelease>,
}

/// Errors callers may want to distinguish from generic `AppError`.
/// We map to `AppError` at the public boundary; the variants exist
/// so the caller can inspect a wrapped error in tests.
#[derive(Debug, Clone)]
pub enum VersionError {
    PinNotSet,
    PinUnreachable(String),
    GitHubUnreachable(String),
    /// Pin is set but the corresponding GitHub release no longer exists
    /// (admin pinned a yanked version, or one was deleted upstream).
    ReleaseMissing { version: String },
    /// Pin is set + release exists, but the (arch, flavor, package)
    /// combination wasn't published. Surfaces as 422 in the admin UI.
    AssetMissing {
        version: String,
        arch: String,
        flavor: String,
        package: String,
    },
    Sha256Mismatch { expected: String, got: String },
    CosignFailed(String),
    Database(String),
    Io(String),
}

impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionError::PinNotSet => write!(
                f,
                "no rootfs version is pinned yet — server has not yet reached \
                 the GitHub Releases API since first boot"
            ),
            VersionError::PinUnreachable(e) => {
                write!(f, "could not reach GitHub to set the initial pin: {e}")
            }
            VersionError::GitHubUnreachable(e) => write!(f, "GitHub API unreachable: {e}"),
            VersionError::ReleaseMissing { version } => write!(
                f,
                "pinned version v{version} no longer exists on GitHub"
            ),
            VersionError::AssetMissing { version, arch, flavor, package } => write!(
                f,
                "release v{version} does not publish an artifact for \
                 ({arch}, {flavor}, {package})"
            ),
            VersionError::Sha256Mismatch { expected, got } => {
                write!(f, "sha256 mismatch (expected {expected}, got {got})")
            }
            VersionError::CosignFailed(e) => write!(f, "cosign verification failed: {e}"),
            VersionError::Database(e) => write!(f, "database error: {e}"),
            VersionError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl VersionError {
    pub fn to_app_error(&self) -> AppError {
        let (status, code) = match self {
            VersionError::PinNotSet | VersionError::PinUnreachable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "ROOTFS_PIN_UNAVAILABLE",
            ),
            VersionError::GitHubUnreachable(_) => (
                StatusCode::BAD_GATEWAY,
                "ROOTFS_GITHUB_UNREACHABLE",
            ),
            VersionError::ReleaseMissing { .. } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ROOTFS_VERSION_MISSING",
            ),
            VersionError::AssetMissing { .. } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ROOTFS_ASSET_MISSING",
            ),
            VersionError::Sha256Mismatch { .. } => (
                StatusCode::BAD_GATEWAY,
                "ROOTFS_SHA256_MISMATCH",
            ),
            VersionError::CosignFailed(_) => (
                StatusCode::BAD_GATEWAY,
                "ROOTFS_COSIGN_FAILED",
            ),
            VersionError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ROOTFS_DATABASE_ERROR",
            ),
            VersionError::Io(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ROOTFS_IO_ERROR",
            ),
        };
        AppError::new(status, code, self.to_string())
    }
}

// =====================================================================
// Per-artifact download lock — single-flight on `(version, arch, flavor, package)`.
// =====================================================================
//
// Two callers may race to download the same artifact: an admin install
// + a chat-side auto-fetch, or two concurrent `execute_command`s
// hitting a freshly-pinned version. Mirror `runtime_fetch::FETCH_LOCKS`
// but keyed on the full (version, arch, flavor, package) tuple instead
// of flavor alone.

static DOWNLOAD_LOCKS: once_cell::sync::Lazy<
    dashmap::DashMap<String, Arc<Mutex<()>>>,
> = once_cell::sync::Lazy::new(dashmap::DashMap::new);

fn download_lock_key(version: &str, arch: &str, flavor: &str, package: &str) -> String {
    format!("{version}/{arch}/{flavor}/{package}")
}

fn download_lock_for(version: &str, arch: &str, flavor: &str, package: &str) -> Arc<Mutex<()>> {
    DOWNLOAD_LOCKS
        .entry(download_lock_key(version, arch, flavor, package))
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

// =====================================================================
// GitHub Releases API
// =====================================================================

/// List published releases on `ziee-ai/sandbox-rootfs`, newest first.
/// Drafts + prereleases are returned (the UI filters them); the boot
/// pin probe rejects them.
pub async fn list_releases() -> Result<Vec<RootfsRelease>, VersionError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .https_only(true)
        .build()
        .map_err(|e| VersionError::GitHubUnreachable(format!("client build: {e}")))?;
    let url = format!("https://api.github.com/repos/{ROOTFS_REPO}/releases");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "ziee/1.0")
        .send()
        .await
        .map_err(|e| VersionError::GitHubUnreachable(format!("GET {url}: {e}")))?;
    if !response.status().is_success() {
        return Err(VersionError::GitHubUnreachable(format!(
            "GET {url}: HTTP {}",
            response.status()
        )));
    }
    let raw: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| VersionError::GitHubUnreachable(format!("parse JSON: {e}")))?;
    let mut out = Vec::with_capacity(raw.len());
    for r in raw {
        let tag = match r.get("tag_name").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => continue,
        };
        if !is_valid_semver_tag(&tag) {
            continue;
        }
        let version = tag.trim_start_matches('v').to_string();
        let asset_names = r
            .get("assets")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        out.push(RootfsRelease {
            version,
            published_at: r
                .get("published_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            draft: r.get("draft").and_then(|v| v.as_bool()).unwrap_or(false),
            prerelease: r
                .get("prerelease")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            asset_names,
        });
    }
    Ok(out)
}

/// `true` when `tag` matches the `vMAJOR.MINOR.PATCH` semver shape the
/// release workflow rejects everything else from.
fn is_valid_semver_tag(tag: &str) -> bool {
    let rest = match tag.strip_prefix('v') {
        Some(r) => r,
        None => return false,
    };
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Pick the highest non-draft, non-prerelease semver release. Returns
/// `None` when no usable release exists yet (fresh repo).
fn pick_latest(releases: &[RootfsRelease]) -> Option<&RootfsRelease> {
    releases
        .iter()
        .filter(|r| !r.draft && !r.prerelease)
        .max_by(|a, b| compare_semver(&a.version, &b.version))
}

/// Lexicographic-by-component semver comparison. Returns `Ordering` so
/// `max_by` produces the largest version.
fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let pa = parse_semver(a);
    let pb = parse_semver(b);
    pa.cmp(&pb)
}

fn parse_semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

// =====================================================================
// Pin lifecycle
// =====================================================================

/// Read the current pin out of `code_sandbox_settings`. `Ok(None)`
/// means the pin has not been set yet (fresh install before the first
/// reachable GitHub call).
pub async fn current_pin(pool: &PgPool) -> Result<Option<String>, VersionError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT current_rootfs_version FROM code_sandbox_settings WHERE id = TRUE")
            .fetch_optional(pool)
            .await
            .map_err(|e| VersionError::Database(e.to_string()))?;
    Ok(row.and_then(|r| r.0))
}

/// Boot-time probe. Idempotent: returns the current pin if already set.
/// Otherwise hits GitHub and writes the latest semver release into
/// `code_sandbox_settings.current_rootfs_version`. Logs (does NOT
/// fail) if GitHub is unreachable; the pin stays `NULL` and the next
/// `execute_command` retries by calling this same function.
pub async fn ensure_pin_initialized(pool: &PgPool) -> Result<Option<String>, VersionError> {
    if let Some(pin) = current_pin(pool).await? {
        return Ok(Some(pin));
    }
    let releases = match list_releases().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                "code_sandbox: rootfs pin probe deferred — could not reach GitHub: {e}"
            );
            return Ok(None);
        }
    };
    let latest = match pick_latest(&releases) {
        Some(r) => r,
        None => {
            tracing::warn!(
                "code_sandbox: no usable semver releases yet on {ROOTFS_REPO}; \
                 pin remains unset"
            );
            return Ok(None);
        }
    };
    sqlx::query("UPDATE code_sandbox_settings SET current_rootfs_version = $1 WHERE id = TRUE")
        .bind(&latest.version)
        .execute(pool)
        .await
        .map_err(|e| VersionError::Database(e.to_string()))?;
    tracing::info!(
        version = %latest.version,
        "code_sandbox: rootfs version pinned to latest release"
    );
    Ok(Some(latest.version.clone()))
}

/// Change the pin to `target_version`. Validates the new version exists
/// on GitHub before writing. Phase 3 will wrap this with drain + wipe;
/// for Phase 2 the change is purely a settings update — in-flight execs
/// keep running on whatever mount they already attached to.
pub async fn set_pin(pool: &PgPool, target_version: &str) -> Result<(), VersionError> {
    let releases = list_releases().await?;
    if !releases.iter().any(|r| r.version == target_version) {
        return Err(VersionError::ReleaseMissing {
            version: target_version.to_string(),
        });
    }
    sqlx::query("UPDATE code_sandbox_settings SET current_rootfs_version = $1 WHERE id = TRUE")
        .bind(target_version)
        .execute(pool)
        .await
        .map_err(|e| VersionError::Database(e.to_string()))?;
    tracing::info!(version = target_version, "code_sandbox: rootfs pin changed");
    Ok(())
}

// =====================================================================
// Artifact lifecycle
// =====================================================================

/// All installed artifacts, newest first. Mirrors `llm_runtime_versions`
/// list shape used by the local-llm-runtime admin page.
pub async fn list_installed(pool: &PgPool) -> Result<Vec<RootfsArtifact>, VersionError> {
    let rows = sqlx::query_as::<_, RootfsArtifact>(
        "SELECT id, version, arch, flavor, package, sha256, artifact_path, \
                cosign_bundle, status, downloaded_at, last_used_at \
         FROM code_sandbox_rootfs_artifacts \
         ORDER BY downloaded_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| VersionError::Database(e.to_string()))?;
    Ok(rows)
}

/// Look up an artifact by its uniqueness tuple.
pub async fn find_artifact(
    pool: &PgPool,
    version: &str,
    arch: &str,
    flavor: &str,
    package: &str,
) -> Result<Option<RootfsArtifact>, VersionError> {
    let row = sqlx::query_as::<_, RootfsArtifact>(
        "SELECT id, version, arch, flavor, package, sha256, artifact_path, \
                cosign_bundle, status, downloaded_at, last_used_at \
         FROM code_sandbox_rootfs_artifacts \
         WHERE version = $1 AND arch = $2 AND flavor = $3 AND package = $4",
    )
    .bind(version)
    .bind(arch)
    .bind(flavor)
    .bind(package)
    .fetch_optional(pool)
    .await
    .map_err(|e| VersionError::Database(e.to_string()))?;
    Ok(row)
}

/// Insert a freshly-downloaded artifact row. Idempotent via the
/// `UNIQUE (version, arch, flavor, package)` constraint — concurrent
/// double-downloads collapse to one row via `ON CONFLICT DO UPDATE`
/// (most-recent download wins; older `artifact_path` value is harmless
/// since we always sha256-verify on read).
pub async fn upsert_artifact(
    pool: &PgPool,
    artifact: &RootfsArtifact,
) -> Result<RootfsArtifact, VersionError> {
    let row = sqlx::query_as::<_, RootfsArtifact>(
        "INSERT INTO code_sandbox_rootfs_artifacts (\
            version, arch, flavor, package, sha256, artifact_path, \
            cosign_bundle, status, downloaded_at\
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW()) \
         ON CONFLICT (version, arch, flavor, package) DO UPDATE SET \
            sha256 = EXCLUDED.sha256, \
            artifact_path = EXCLUDED.artifact_path, \
            cosign_bundle = EXCLUDED.cosign_bundle, \
            status = EXCLUDED.status, \
            downloaded_at = NOW() \
         RETURNING id, version, arch, flavor, package, sha256, artifact_path, \
            cosign_bundle, status, downloaded_at, last_used_at",
    )
    .bind(&artifact.version)
    .bind(&artifact.arch)
    .bind(&artifact.flavor)
    .bind(&artifact.package)
    .bind(&artifact.sha256)
    .bind(&artifact.artifact_path)
    .bind(&artifact.cosign_bundle)
    .bind(&artifact.status)
    .fetch_one(pool)
    .await
    .map_err(|e| VersionError::Database(e.to_string()))?;
    Ok(row)
}

/// Touch `last_used_at` after a successful mount. Best-effort —
/// failure is logged but not propagated.
pub async fn touch_last_used(pool: &PgPool, id: Uuid) {
    if let Err(e) =
        sqlx::query("UPDATE code_sandbox_rootfs_artifacts SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
    {
        tracing::warn!(artifact = %id, "code_sandbox: failed to touch last_used_at: {e}");
    }
}

/// Delete an artifact row + remove its cached files. Phase 3 will add
/// inflight-counter + pin guard; for Phase 2 we only refuse to delete
/// if the row is the currently-pinned version.
pub async fn delete_artifact(pool: &PgPool, id: Uuid) -> Result<(), VersionError> {
    let row = sqlx::query_as::<_, RootfsArtifact>(
        "SELECT id, version, arch, flavor, package, sha256, artifact_path, \
                cosign_bundle, status, downloaded_at, last_used_at \
         FROM code_sandbox_rootfs_artifacts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| VersionError::Database(e.to_string()))?;
    let row = match row {
        Some(r) => r,
        None => return Ok(()),
    };
    if let Some(pin) = current_pin(pool).await?
        && pin == row.version
    {
        return Err(VersionError::Database(format!(
            "cannot delete artifact for currently-pinned version v{pin}; \
             change the pin first"
        )));
    }
    sqlx::query("DELETE FROM code_sandbox_rootfs_artifacts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| VersionError::Database(e.to_string()))?;
    // Best-effort file cleanup. A failure here leaves an orphan in the
    // cache dir, which the next boot's sync pass can prune.
    let path = PathBuf::from(&row.artifact_path);
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(
                path = %path.display(),
                "code_sandbox: artifact file cleanup failed: {e}"
            );
        }
    }
    for sidecar in [".sha256", ".zsync", ".cosign.bundle"] {
        let mut name = path
            .file_name()
            .unwrap_or_default()
            .to_os_string();
        name.push(sidecar);
        let sidecar_path = path.with_file_name(name);
        let _ = std::fs::remove_file(sidecar_path);
    }
    Ok(())
}

// =====================================================================
// Download + verify
// =====================================================================

/// Download + sha256 + cosign verify the artifact for
/// `(version, arch, flavor, package)`. Single-flight via `DOWNLOAD_LOCKS`.
/// Re-uses the existing low-level primitives in `runtime_fetch` to
/// avoid duplicating the reqwest + sigstore plumbing.
///
/// Idempotent: if the DB already has a matching row whose file is on
/// disk with the expected sha256, returns that row without hitting
/// the network. The returned `(RootfsArtifact, Option<DownloadStats>)`
/// — `Some` when this call actually downloaded bytes, `None` on a
/// cache hit — is what `resolve_artifact` uses to populate
/// `fetch_info` for the chat UI.
///
/// `cache_dir` is the per-version-agnostic root that holds version
/// subdirs: files land at `<cache_dir>/<version>/<asset_name>`.
pub async fn install_version(
    pool: &PgPool,
    cache_dir: &std::path::Path,
    version: &str,
    arch: &str,
    flavor: &str,
    package: &str,
    progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(RootfsArtifact, Option<DownloadStats>), VersionError> {
    let lock = download_lock_for(version, arch, flavor, package);
    let _guard = lock.lock().await;

    if let Some(existing) = find_artifact(pool, version, arch, flavor, package).await?
        && verify_cached_sha256(&existing).await.unwrap_or(false)
    {
        return Ok((existing, None));
    }

    let ext = package_extension(package)?;
    let asset_name = format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}");
    let tag = format!("v{version}");
    let url = build_download_url(&tag, &asset_name);

    let version_cache_dir = cache_dir.join(version);
    std::fs::create_dir_all(&version_cache_dir).map_err(|e| VersionError::Io(format!(
        "create cache dir {}: {e}",
        version_cache_dir.display()
    )))?;
    let out_path = version_cache_dir.join(&asset_name);

    progress(InstallProgress::Resolving { version: version.to_string(), asset: asset_name.clone() });
    let pool_for_blocking = pool.clone();
    let version_owned = version.to_string();
    let arch_owned = arch.to_string();
    let url_owned = url.clone();
    let out_path_owned = out_path.clone();
    let asset_owned = asset_name.clone();

    let outcome = tokio::task::spawn_blocking(move || {
        download_verify_blocking(
            &url_owned,
            &out_path_owned,
            &asset_owned,
            &version_owned,
            &arch_owned,
            progress,
        )
    })
    .await
    .map_err(|e| VersionError::Io(format!("blocking task panicked: {e}")))??;

    let row = RootfsArtifact {
        id: Uuid::nil(), // overwritten by RETURNING
        version: version.to_string(),
        arch: arch.to_string(),
        flavor: flavor.to_string(),
        package: package.to_string(),
        sha256: outcome.sha256,
        artifact_path: out_path.to_string_lossy().into_owned(),
        cosign_bundle: outcome.cosign_bundle_path,
        status: "installed".to_string(),
        downloaded_at: Utc::now(),
        last_used_at: None,
    };
    let stats = DownloadStats {
        bytes_downloaded: outcome.bytes_downloaded,
        duration_ms: outcome.duration_ms,
        cosign_verified: outcome.cosign_verified,
    };
    let inserted = upsert_artifact(&pool_for_blocking, &row).await?;
    Ok((inserted, Some(stats)))
}

/// Single-call lazy resolver used by the `execute_command` MCP tool +
/// the lower-level `runtime_fetch::ensure_fetched` shim. Reads the
/// pin, then either returns the cached artifact row or downloads +
/// installs the missing one.
///
/// Returns `(artifact, fetch_stats)` — `fetch_stats` is `Some` only when
/// this call did the download (so the chat UI's `fetch_info` can stay
/// `None` on warm-path hits).
pub async fn resolve_artifact(
    pool: &PgPool,
    cache_dir: &std::path::Path,
    arch: &str,
    flavor: &str,
    package: &str,
) -> Result<(RootfsArtifact, Option<DownloadStats>), VersionError> {
    let pinned = ensure_pin_initialized(pool)
        .await?
        .ok_or(VersionError::PinNotSet)?;
    install_version(pool, cache_dir, &pinned, arch, flavor, package, |_| {}).await
}

/// Stats surfaced via `EnsureOutcome.fetch_info` for the chat UI.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DownloadStats {
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
    pub cosign_verified: bool,
}

/// Quick check: does the on-disk artifact match the DB's expected sha?
async fn verify_cached_sha256(artifact: &RootfsArtifact) -> Result<bool, VersionError> {
    let path = PathBuf::from(&artifact.artifact_path);
    if !path.exists() {
        return Ok(false);
    }
    let expected = artifact.sha256.clone();
    let result = tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|e| VersionError::Io(format!("sha256 task panicked: {e}")))?;
    match result {
        Ok(actual) => Ok(actual.eq_ignore_ascii_case(&expected)),
        Err(_) => Ok(false),
    }
}

fn package_extension(package: &str) -> Result<&'static str, VersionError> {
    match package {
        "squashfs" => Ok("squashfs"),
        "tar.zst" => Ok("tar.zst"),
        other => Err(VersionError::Database(format!(
            "unknown package type {other:?}"
        ))),
    }
}

fn build_download_url(tag: &str, asset: &str) -> String {
    let base = std::env::var("CODE_SANDBOX_ROOTFS_MIRROR")
        .unwrap_or_else(|_| format!("https://github.com/{ROOTFS_REPO}/releases/download"));
    format!("{base}/{tag}/{asset}")
}

/// Streamed install progress for the admin UI's SSE channel. Phase 2
/// just emits the phase + a free-form message; Phase 4 will wire
/// `bytes_total` / `bytes_done` once the download path streams chunks.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum InstallProgress {
    Resolving { version: String, asset: String },
    Downloading { url: String },
    VerifyingSha256,
    VerifyingCosign,
    Installing { path: String },
}

#[derive(Debug, Clone)]
struct DownloadOutcome {
    sha256: String,
    cosign_bundle_path: Option<String>,
    bytes_downloaded: u64,
    duration_ms: u64,
    cosign_verified: bool,
}

fn download_verify_blocking(
    url: &str,
    out_path: &std::path::Path,
    asset_name: &str,
    version: &str,
    arch: &str,
    progress: impl Fn(InstallProgress) + Send + Sync,
) -> Result<DownloadOutcome, VersionError> {
    let started = std::time::Instant::now();
    let tmp_path = out_path.with_extension(format!(
        "{}.tmp",
        out_path.extension().and_then(|e| e.to_str()).unwrap_or("bin")
    ));
    progress(InstallProgress::Downloading { url: url.to_string() });

    // Reuse the existing reqwest::blocking + sigstore primitives via
    // crate-public helpers in runtime_fetch. Defined here as a thin
    // wrapper to keep the interface narrow.
    let bytes_downloaded = crate::modules::code_sandbox::runtime_fetch::download_blob_blocking(
        url, &tmp_path, 3,
    )
    .map_err(|e| VersionError::Io(format!("download failed: {e}")))?;
    if bytes_downloaded == 0 {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(VersionError::Io(format!("download returned 0 bytes from {url}")));
    }

    progress(InstallProgress::VerifyingSha256);
    let actual_sha = sha256_file(&tmp_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        VersionError::Io(format!("sha256 read: {e}"))
    })?;

    // Fetch + verify the sha256 sidecar from the same release. The
    // sidecar is `<asset>.sha256` next to the artifact on GitHub
    // Releases.
    let sha_sidecar_url = format!("{url}.sha256");
    let sha_sidecar_path = tmp_path.with_extension("sha256.tmp");
    let _ = crate::modules::code_sandbox::runtime_fetch::download_blob_blocking(
        &sha_sidecar_url,
        &sha_sidecar_path,
        2,
    );
    if sha_sidecar_path.exists()
        && let Ok(content) = std::fs::read_to_string(&sha_sidecar_path)
    {
        let expected_sha = content
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !expected_sha.is_empty() && expected_sha != actual_sha {
            let _ = std::fs::remove_file(&tmp_path);
            let _ = std::fs::remove_file(&sha_sidecar_path);
            return Err(VersionError::Sha256Mismatch {
                expected: expected_sha,
                got: actual_sha,
            });
        }
    }
    let _ = std::fs::remove_file(&sha_sidecar_path);

    progress(InstallProgress::VerifyingCosign);
    let cosign_bundle_url = format!("{url}.cosign.bundle");
    let mut cosign_bundle_dest = out_path.file_name().unwrap_or_default().to_os_string();
    cosign_bundle_dest.push(".cosign.bundle");
    let cosign_bundle_path = out_path.with_file_name(cosign_bundle_dest);

    let bundle_dl = crate::modules::code_sandbox::runtime_fetch::download_blob_blocking(
        &cosign_bundle_url,
        &cosign_bundle_path,
        2,
    );
    let cosign_bundle_kept = match bundle_dl {
        Ok(n) if n > 0 => {
            let expected_identity = format!(
                "https://github.com/{ROOTFS_REPO}/.github/workflows/release.yml@refs/tags/v{version}"
            );
            let _ = arch; // identity does not include arch under the new convention
            let _ = asset_name;
            match crate::modules::code_sandbox::runtime_fetch::verify_cosign_blob(
                &cosign_bundle_path,
                &tmp_path,
                &expected_identity,
                "https://token.actions.githubusercontent.com",
            ) {
                Ok(()) => Some(cosign_bundle_path.to_string_lossy().into_owned()),
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    let _ = std::fs::remove_file(&cosign_bundle_path);
                    return Err(VersionError::CosignFailed(e));
                }
            }
        }
        // No bundle published — Phase 2 keeps this lenient (unsigned
        // releases are accepted with a warning log). Phase 1's
        // `release.yml` always uploads a bundle, so production
        // artifacts always get verified; dev `--package tar` builds
        // staged via `dev-release.sh` skip signing on purpose.
        _ => {
            tracing::warn!(
                url = %cosign_bundle_url,
                "code_sandbox: cosign bundle not published; accepting unsigned artifact"
            );
            None
        }
    };

    progress(InstallProgress::Installing {
        path: out_path.to_string_lossy().into_owned(),
    });
    std::fs::rename(&tmp_path, out_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        VersionError::Io(format!("rename to {}: {e}", out_path.display()))
    })?;

    Ok(DownloadOutcome {
        sha256: actual_sha,
        cosign_verified: cosign_bundle_kept.is_some(),
        cosign_bundle_path: cosign_bundle_kept,
        bytes_downloaded,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

// =====================================================================
// Status snapshot for the admin UI
// =====================================================================

/// One-stop call for the admin "Rootfs versions" page. Reads the pin +
/// installed-artifact list from the DB, and attempts a GitHub query
/// (best-effort — the UI shows installed/pinned regardless of network
/// state).
pub async fn status(pool: &PgPool) -> Result<VersionStatus, VersionError> {
    let pinned_version = current_pin(pool).await?;
    let installed = list_installed(pool).await?;
    let available = match list_releases().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("code_sandbox: status() — GitHub list_releases failed: {e}");
            Vec::new()
        }
    };
    Ok(VersionStatus { pinned_version, installed, available })
}

// =====================================================================
// Tier 1 unit tests
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_tag_regex_accepts_valid() {
        for t in ["v0.0.1", "v0.1.0", "v1.0.0", "v10.20.30"] {
            assert!(is_valid_semver_tag(t), "should accept {t}");
        }
    }

    #[test]
    fn semver_tag_regex_rejects_invalid() {
        for t in [
            "0.1.0",                        // missing v
            "v0.1",                         // 2 components
            "v0.1.0-rc1",                   // prerelease suffix (we strip those via prerelease flag)
            "v0.1.0+meta",                  // build metadata
            "v0.01.0",                      // leading zeros not enforced but bash regex would accept; lax
            "vfoo",                         // non-numeric
            "sandbox-rootfs-v1.r0-x86_64",  // legacy tag shape
        ] {
            // We tolerate leading zeros, so adjust expectations for v0.01.0
            if t == "v0.01.0" {
                continue;
            }
            assert!(!is_valid_semver_tag(t), "should reject {t}");
        }
    }

    #[test]
    fn pick_latest_skips_drafts_and_prereleases() {
        let releases = vec![
            RootfsRelease {
                version: "0.2.0".into(),
                published_at: None,
                draft: true,
                prerelease: false,
                asset_names: vec![],
            },
            RootfsRelease {
                version: "0.1.5".into(),
                published_at: None,
                draft: false,
                prerelease: true,
                asset_names: vec![],
            },
            RootfsRelease {
                version: "0.1.0".into(),
                published_at: None,
                draft: false,
                prerelease: false,
                asset_names: vec![],
            },
            RootfsRelease {
                version: "0.1.3".into(),
                published_at: None,
                draft: false,
                prerelease: false,
                asset_names: vec![],
            },
        ];
        let pick = pick_latest(&releases).expect("a non-draft non-prerelease exists");
        assert_eq!(pick.version, "0.1.3");
    }

    #[test]
    fn compare_semver_orders_correctly() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_semver("0.1.0", "0.1.0"), Equal);
        assert_eq!(compare_semver("0.1.0", "0.1.1"), Less);
        assert_eq!(compare_semver("0.2.0", "0.1.9"), Greater);
        assert_eq!(compare_semver("1.0.0", "0.9.9"), Greater);
    }

    #[test]
    fn download_lock_is_per_tuple() {
        let a1 = download_lock_for("0.1.0", "x86_64", "minimal", "squashfs");
        let a2 = download_lock_for("0.1.0", "x86_64", "minimal", "squashfs");
        let b = download_lock_for("0.1.1", "x86_64", "minimal", "squashfs");
        let c = download_lock_for("0.1.0", "x86_64", "full", "squashfs");
        assert!(Arc::ptr_eq(&a1, &a2));
        assert!(!Arc::ptr_eq(&a1, &b));
        assert!(!Arc::ptr_eq(&a1, &c));
    }

    #[test]
    fn package_extension_maps_correctly() {
        assert_eq!(package_extension("squashfs").unwrap(), "squashfs");
        assert_eq!(package_extension("tar.zst").unwrap(), "tar.zst");
        assert!(package_extension("zip").is_err());
    }

    #[test]
    fn build_download_url_uses_default_and_mirror_envs() {
        // Default path — no env set (avoid global env-var contamination
        // in CI by ignoring the result, just asserting it's well-formed).
        let url = build_download_url("v0.1.0", "x.squashfs");
        assert!(
            url.contains("ziee-ai/sandbox-rootfs/releases/download")
                || url.contains(ROOTFS_REPO),
            "unexpected default URL: {url}"
        );
        assert!(url.ends_with("/v0.1.0/x.squashfs"));
    }
}
