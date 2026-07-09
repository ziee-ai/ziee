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
//!   - `execute_command` lazy-fetch path (`install_version`, via
//!     `runtime_fetch::ensure_fetched` / `sandbox`)
//!   - admin handlers (`list_releases`, `install_version`, `set_pin`,
//!     `delete_artifact`, `status`)
//!
//! Phase 2 lands the DB-row + GitHub-API + download lifecycle.
//! Phase 3 will layer per-mount inflight counters + drain/wipe on top
//! of the same `set_pin` entry point.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modules::code_sandbox::config::SandboxAvailability;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
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
/// versions" page.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VersionStatus {
    pub pinned_version: Option<String>,
    pub installed: Vec<RootfsArtifact>,
    /// Only populated when GitHub is reachable (best-effort).
    pub available: Vec<RootfsRelease>,
    /// One entry per live mount (registered after a successful
    /// `runtime_mount::ensure_rootfs_ready`). The admin UI uses
    /// this to render per-row "in-flight" counts + draining
    /// indicators on rows being phased out by a set-pin.
    pub draining: Vec<DrainEntry>,
    /// Count of per-conversation workspace dirs that would be
    /// touched by a major-bump wipe. Read by the major-bump confirm
    /// modal so the admin sees blast radius before clicking through.
    pub conversation_count: usize,
    /// Same as above, for per-MCP-server workspaces under
    /// `<workspace_root>/mcp/<server_id>/`.
    pub mcp_server_workspace_count: usize,
    /// The local host's CPU arch (`"x86_64"` | `"aarch64"`). Authoritative —
    /// the admin UI uses this to offer the artifact this machine can run,
    /// instead of guessing from installed artifacts (which is empty on a fresh
    /// host).
    pub host_arch: String,
    /// The rootfs package format the local backend can actually mount
    /// (`"squashfs"` on Linux/macOS, `"tar.zst"` on Windows/WSL2). Authoritative
    /// — prevents a fresh Windows host from pre-fetching an unmountable squashfs.
    pub host_package: String,
    /// Whether `code_sandbox` is initialized, and if not, the machine-readable
    /// reason. `ready` when the sandbox is registered (full status); otherwise a
    /// degraded snapshot — the GitHub `available` catalog with empty
    /// `installed`/`pinned` — so the admin UI can explain WHY installing/mounting
    /// is unavailable instead of showing a blanket error.
    pub availability: SandboxAvailability,
}

/// Per-mount snapshot exposed via `status()` and the admin UI.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DrainEntry {
    pub artifact_id: Uuid,
    pub version: String,
    pub arch: String,
    pub flavor: String,
    pub inflight_exec: usize,
    pub inflight_mcp: usize,
}

/// Errors callers may want to distinguish from generic `AppError`.
/// We map to `AppError` at the public boundary; the variants exist
/// so the caller can inspect a wrapped error in tests.
#[derive(Debug, Clone)]
pub enum VersionError {
    #[allow(dead_code)]
    PinNotSet,
    #[allow(dead_code)]
    PinUnreachable(String),
    GitHubUnreachable(String),
    /// Pin is set but the corresponding GitHub release no longer exists
    /// (admin pinned a yanked version, or one was deleted upstream).
    ReleaseMissing { version: String },
    /// Pin is set + release exists, but the (arch, flavor, package)
    /// combination wasn't published. Surfaces as 422 in the admin UI.
    #[allow(dead_code)]
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
    /// Refused — at least one mount of this artifact is still
    /// in-flight (an exec session or a sandboxed MCP server is
    /// holding it). Plan 5 Phase 3.
    ArtifactInUse {
        version: String,
        arch: String,
        flavor: String,
        inflight: usize,
    },
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
            VersionError::ArtifactInUse { version, arch, flavor, inflight } => write!(
                f,
                "artifact v{version} ({arch}-{flavor}) has {inflight} live session(s) — \
                 cannot delete until they drain"
            ),
        }
    }
}

impl VersionError {
    pub fn to_app_error(&self) -> AppError {
        let (status, code) = match self {
            VersionError::PinNotSet | VersionError::PinUnreachable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_ROOTFS_UNAVAILABLE",
            ),
            VersionError::ArtifactInUse { .. } => {
                (StatusCode::CONFLICT, "SANDBOX_ROOTFS_ARTIFACT_IN_USE")
            }
            VersionError::GitHubUnreachable(_) => (
                StatusCode::BAD_GATEWAY,
                "SANDBOX_ROOTFS_GITHUB_UNREACHABLE",
            ),
            VersionError::ReleaseMissing { .. } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "SANDBOX_ROOTFS_VERSION_MISSING",
            ),
            VersionError::AssetMissing { .. } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "SANDBOX_ROOTFS_ASSET_MISSING",
            ),
            VersionError::Sha256Mismatch { .. } => (
                StatusCode::BAD_GATEWAY,
                "SANDBOX_ROOTFS_SHA256_MISMATCH",
            ),
            VersionError::CosignFailed(_) => (
                StatusCode::BAD_GATEWAY,
                "SANDBOX_ROOTFS_COSIGN_FAILED",
            ),
            VersionError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "SANDBOX_ROOTFS_DATABASE_ERROR",
            ),
            VersionError::Io(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "SANDBOX_ROOTFS_IO_ERROR",
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
    // Retry transient failures (network/timeout, HTTP 5xx, 429) with
    // exponential backoff so a single hiccup doesn't fail the version probe.
    let response = {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let r = client
                .get(&url)
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "ziee/1.0")
                .send()
                .await;
            let transient = match &r {
                Ok(resp) => resp.status().is_server_error() || resp.status().as_u16() == 429,
                Err(_) => true,
            };
            if transient && attempt < MAX_ATTEMPTS {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::warn!(
                    "GitHub releases {url}: transient failure, retrying in {delay:?} (attempt {attempt}/{MAX_ATTEMPTS})"
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            break r.map_err(|e| VersionError::GitHubUnreachable(format!("GET {url}: {e}")))?;
        }
    };
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

/// `true` when `tag` matches the `vMAJOR.MINOR.PATCH[-PRERELEASE]`
/// semver shape the release workflow rejects everything else from.
/// Audit B9: enforce semver §2 "no leading zeroes on numeric
/// identifiers" so `v01.2.3` is rejected; allow optional prerelease
/// suffix (`-alpha`, `-rc.1`) so the existing `v0.0.2-alpha` rootfs
/// release tags validate; require non-empty prerelease when `-` is
/// present (`v1.2.3-` is rejected).
fn is_valid_semver_tag(tag: &str) -> bool {
    let rest = match tag.strip_prefix('v') {
        Some(r) => r,
        None => return false,
    };
    let (core, prerelease) = match rest.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (rest, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let core_ok = parts.iter().all(|p| {
        if p.is_empty() {
            return false;
        }
        // No leading zeros, except the literal "0".
        if p.len() > 1 && p.starts_with('0') {
            return false;
        }
        p.chars().all(|c| c.is_ascii_digit())
    });
    if !core_ok {
        return false;
    }
    match prerelease {
        None => true,
        Some(pre) => {
            if pre.is_empty() {
                return false;
            }
            // Semver §9: prerelease is one or more dot-separated
            // identifiers; each identifier is `[0-9A-Za-z-]+`,
            // numeric identifiers cannot have leading zeros.
            pre.split('.').all(|id| {
                if id.is_empty() {
                    return false;
                }
                if id.chars().all(|c| c.is_ascii_digit())
                    && id.len() > 1
                    && id.starts_with('0')
                {
                    return false;
                }
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
        }
    }
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
    // Explicit pin override. Lets an operator (or the test suite) pin a
    // specific version deterministically instead of relying on
    // "latest" discovery — required for prerelease-style tags (e.g.
    // `0.0.2-alpha`) that the semver `pick_latest` comparison can't
    // order. Trusted as-is here (no GitHub round-trip); a wrong value
    // surfaces as a clear AssetMissing on the first fetch.
    if let Ok(forced) = std::env::var("CODE_SANDBOX_PIN_VERSION") {
        let forced = forced.trim().trim_start_matches('v').to_string();
        if !forced.is_empty() {
            sqlx::query(
                "UPDATE code_sandbox_settings SET current_rootfs_version = $1 WHERE id = TRUE",
            )
            .bind(&forced)
            .execute(pool)
            .await
            .map_err(|e| VersionError::Database(e.to_string()))?;
            tracing::info!(
                version = %forced,
                "code_sandbox: rootfs pin set from CODE_SANDBOX_PIN_VERSION"
            );
            return Ok(Some(forced));
        }
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
///
/// Audit convergence pass: explicit `LIMIT 1024` guards against an
/// unbounded materialization if the artifacts table ever grows past
/// the operator's intuition (each row is small and human-managed via
/// the admin UI, so 1024 is comfortably above any sane deployment;
/// hitting it means an admin walked into a problem and the UI's
/// pagination decision needs revisiting, not a silent OOM).
pub async fn list_installed(pool: &PgPool) -> Result<Vec<RootfsArtifact>, VersionError> {
    let rows = sqlx::query_as::<_, RootfsArtifact>(
        "SELECT id, version, arch, flavor, package, sha256, artifact_path, \
                cosign_bundle, status, downloaded_at, last_used_at \
         FROM code_sandbox_rootfs_artifacts \
         ORDER BY downloaded_at DESC \
         LIMIT 1024",
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

    // Refuse if any live mount of this artifact still has inflight
    // execs / MCP sessions. The delete handler is the second 409
    // guard after the pin check.
    //
    // Audit B5: avoid a race between the `inflight()` check and the
    // actual eviction by atomically REMOVING the registry entry
    // first — that closes the door against new `acquire_inflight`
    // calls. Then re-check inflight on the removed handle; if a
    // racing caller incremented in the brief overlap window, re-
    // insert + refuse.
    if let Some((_, mounted)) = MOUNTED_ARTIFACTS.remove(&row.id) {
        let live = mounted.inflight();
        if live > 0 {
            // Lost the race: a caller grabbed an inflight guard
            // between our check and the removal. Put it back.
            MOUNTED_ARTIFACTS.insert(row.id, mounted);
            return Err(VersionError::ArtifactInUse {
                version: row.version.clone(),
                arch: row.arch.clone(),
                flavor: row.flavor.clone(),
                inflight: live,
            });
        }
        // Tear down the backend mount (squashfuse unmount on Linux,
        // VM stop on mac_vm, distro unregister on wsl2). Returns an
        // EvictOutcome (not Result) — surface a warn if the backend
        // claims nothing was cached even though we just removed a
        // registry entry, but proceed with the DB delete either way
        // (re-inserting the registry entry would just block a later
        // operator retry of the delete on the same id).
        let outcome = crate::modules::code_sandbox::backend::active()
            .evict_artifact(&mounted.mount_dir, &mounted.flavor, &mounted.version)
            .await;
        if !outcome.was_cached {
            tracing::warn!(
                artifact = %row.id,
                version = %row.version,
                flavor = %row.flavor,
                "code_sandbox: delete_artifact backend reported no cache hit \
                 (registry/disk drift); continuing with DB delete"
            );
        }
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
/// cache hit — is what the lazy-fetch path uses to populate
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

    // Pre-staged / air-gapped adoption: if the artifact file is already
    // on disk but the DB has no matching row (operator manually copied
    // a squashfs into the cache dir — the documented air-gapped flow —
    // or a prior install populated the cache and the DB row was later
    // wiped), adopt it WITHOUT re-downloading. Trust model matches the
    // air-gapped story: the per-version cache dir is server-owned, so a
    // file there was placed deliberately; we record its sha256 as the
    // integrity baseline and re-verify the cosign bundle if one sits
    // beside it. This is also what lets the test suite share a single
    // download across many fresh-DB Tier-6 servers.
    if out_path.is_file() {
        let p = out_path.clone();
        let sha = tokio::task::spawn_blocking(move || sha256_file(&p))
            .await
            .map_err(|e| VersionError::Io(format!("sha256 task panicked: {e}")))?
            .map_err(|e| VersionError::Io(format!("sha256 {}: {e}", out_path.display())))?;
        let bundle = version_cache_dir.join(format!("{asset_name}.cosign.bundle"));
        let cosign_bundle = bundle
            .is_file()
            .then(|| bundle.to_string_lossy().into_owned());
        let row = RootfsArtifact {
            id: Uuid::nil(),
            version: version.to_string(),
            arch: arch.to_string(),
            flavor: flavor.to_string(),
            package: package.to_string(),
            sha256: sha,
            artifact_path: out_path.to_string_lossy().into_owned(),
            cosign_bundle,
            status: "installed".to_string(),
            downloaded_at: Utc::now(),
            last_used_at: None,
        };
        let inserted = upsert_artifact(pool, &row).await?;
        tracing::info!(
            version, arch, flavor, package,
            "code_sandbox: adopted pre-staged rootfs artifact from cache (no download)"
        );
        return Ok((inserted, None));
    }

    // Surface the dev-only mirror override so it's never silent: in a debug
    // build with CODE_SANDBOX_ROOTFS_MIRROR set, downloads do NOT hit GitHub.
    // Emitted here (after the cache-hit + pre-staged-adoption early-returns) so
    // it only fires when a real download is about to happen. Compiled out of
    // release builds (where the mirror is never honored), so production is
    // unaffected.
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("CODE_SANDBOX_ROOTFS_MIRROR")
        && !mirror.trim().is_empty()
    {
        tracing::warn!(
            mirror = %mirror,
            "code_sandbox: CODE_SANDBOX_ROOTFS_MIRROR is set — downloading {asset_name} from the \
             dev mirror, NOT GitHub Releases. Unset CODE_SANDBOX_ROOTFS_MIRROR to use GitHub."
        );
    }

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

/// Build the artifact download URL for a release `tag` + `asset` name.
///
/// Defaults to the real GitHub Releases host. In **debug builds only**
/// the `CODE_SANDBOX_ROOTFS_MIRROR` env var may override the whole download
/// base (the mirror serves `{base}/{tag}/{asset}` at its root, unlike the
/// host-only override in `llm_local_runtime`'s `engine::download::release_base_url`)
/// so the dev-release loopback mirror (`scripts/dev-release.sh`) and the
/// integration-test `mirror_fixture` can serve artifacts without cutting a
/// real release tag. The env read is compiled out of release builds via
/// `cfg!(debug_assertions)` — the same gating as `release_base_url` — so a
/// production binary can never be redirected away from GitHub, even if the
/// env var is set.
fn build_download_url(tag: &str, asset: &str) -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("CODE_SANDBOX_ROOTFS_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return format!("{mirror}/{tag}/{asset}");
        }
    }
    format!("https://github.com/{ROOTFS_REPO}/releases/download/{tag}/{asset}")
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
        // No bundle published. `release.yml` always uploads a bundle, so in a
        // RELEASE build a missing bundle means a tampered / hijacked-mirror
        // artifact (and the sha256 sidecar comes from the same host, so it's no
        // defense) — refuse it. DEBUG builds (dev-release.sh `signed=false`,
        // the mirror_fixture, local `--package tar`) accept unsigned with a warn.
        _ => {
            #[cfg(not(debug_assertions))]
            {
                let _ = std::fs::remove_file(&tmp_path);
                let _ = std::fs::remove_file(&cosign_bundle_path);
                return Err(VersionError::CosignFailed(
                    "cosign bundle not published; refusing unsigned rootfs artifact".to_string(),
                ));
            }
            #[cfg(debug_assertions)]
            {
                tracing::warn!(
                    url = %cosign_bundle_url,
                    "code_sandbox: cosign bundle not published; accepting unsigned artifact (debug build only)"
                );
                None
            }
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
    let draining: Vec<DrainEntry> = list_mounted_artifacts()
        .into_iter()
        .map(|m| {
            let (e, mcp) = m.inflight_breakdown();
            DrainEntry {
                artifact_id: m.artifact_id,
                version: m.version.clone(),
                arch: m.arch.clone(),
                flavor: m.flavor.clone(),
                inflight_exec: e,
                inflight_mcp: mcp,
            }
        })
        .collect();
    let (conversation_count, mcp_server_workspace_count) =
        match crate::modules::code_sandbox::config::get_state() {
            Some(state) => count_workspaces(&state.workspace_root),
            None => (0, 0),
        };
    Ok(VersionStatus {
        pinned_version,
        installed,
        available,
        draining,
        conversation_count,
        mcp_server_workspace_count,
        host_arch: std::env::consts::ARCH.to_string(),
        // Mirrors runtime_mount's per-OS format selection so the UI offers what
        // the local backend can mount.
        host_package: if cfg!(target_os = "windows") {
            "tar.zst"
        } else {
            "squashfs"
        }
        .to_string(),
        // `status()` is only reached when the sandbox is fully initialized.
        availability: SandboxAvailability::Ready,
    })
}

/// The host arch string the admin UI keys the "install this" affordance off of.
/// Kept identical to `status()`'s computation so degraded + initialized agree.
fn host_arch() -> String {
    std::env::consts::ARCH.to_string()
}

/// The rootfs package format the local backend can mount. Kept identical to
/// `status()`'s computation.
fn host_package() -> String {
    if cfg!(target_os = "windows") {
        "tar.zst"
    } else {
        "squashfs"
    }
    .to_string()
}

/// Build a degraded `VersionStatus` (pure — no DB, no network). Used when
/// `code_sandbox` is not initialized: the GitHub `available` catalog is still
/// shown, but `installed`/`pinned`/`draining` are empty and `availability`
/// names the reason. Split from `available_only` so it is unit-testable without
/// touching the network.
fn build_degraded(availability: SandboxAvailability, available: Vec<RootfsRelease>) -> VersionStatus {
    VersionStatus {
        pinned_version: None,
        installed: Vec::new(),
        available,
        draining: Vec::new(),
        conversation_count: 0,
        mcp_server_workspace_count: 0,
        host_arch: host_arch(),
        host_package: host_package(),
        availability,
    }
}

/// Degraded status for the admin "Rootfs versions" page when the sandbox isn't
/// initialized. Fetches the GitHub catalog best-effort (empty on network
/// failure, exactly as `status()` does) and tags it with the reason so the UI
/// degrades gracefully instead of erroring.
pub async fn available_only(availability: SandboxAvailability) -> VersionStatus {
    let available = list_releases().await.unwrap_or_else(|e| {
        tracing::debug!("code_sandbox: available_only() — GitHub list_releases failed: {e}");
        Vec::new()
    });
    build_degraded(availability, available)
}

/// Tally (per-conversation, per-MCP-server) workspace dirs that a
/// major-bump wipe would walk. Both classes match the layout
/// `wipe_install_caches_in_root` walks: direct children of
/// `workspace_root` (minus `attachments` / `identity` / `mcp`) +
/// `<workspace_root>/mcp/<server_id>/` entries.
fn count_workspaces(workspace_root: &std::path::Path) -> (usize, usize) {
    let mut conv = 0usize;
    let mut mcp = 0usize;
    if !workspace_root.is_dir() {
        return (conv, mcp);
    }
    let entries = match std::fs::read_dir(workspace_root) {
        Ok(e) => e,
        Err(_) => return (conv, mcp),
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_dir() {
            continue;
        }
        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name == "mcp" {
            if let Ok(inner) = std::fs::read_dir(entry.path()) {
                for sub in inner.flatten() {
                    if sub.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        mcp += 1;
                    }
                }
            }
            continue;
        }
        if name == "attachments" || name == "identity" {
            continue;
        }
        conv += 1;
    }
    (conv, mcp)
}

// =====================================================================
// Phase 3 — swap-while-running mechanics
// =====================================================================
//
// Two coupled mechanisms:
//
//   1. Per-artifact inflight counter.  Bumped + decremented around
//      every live use of a mounted artifact — both `execute_command`
//      sessions and long-lived sandboxed-MCP-server sessions — via
//      RAII `InflightGuard`s.  When the counter hits zero AND the
//      artifact is no longer the pin, the version manager evicts the
//      mount.
//
//   2. Pin-change swap.  `set_pin_with_drain` chooses a wipe policy
//      from the semver diff (major bump => WipeCachesOnDrain, else
//      Preserve), updates the pin atomically, and spawns a per-old-
//      artifact drain task that:
//        - waits on the artifact's `drained` Notify until inflight==0,
//        - calls `backend::active().evict_artifact(...)`,
//        - if WipeCachesOnDrain, walks the workspace tree (both
//          `<workspace_root>/<conv_uuid>/` and
//          `<workspace_root>/mcp/<server_id>/`) and `rm -rf`s the
//          curated install-cache subdir list.  Drops a
//          `.rootfs-upgraded` sentinel in each so the next
//          `execute_command` reads + unlinks it and prepends a
//          system note to the tool result.
//
// The actual `backend::active().evict_artifact` plumbing lives in
// `code_sandbox::backend` (per-backend impls); this module owns the
// counter + drain coordination.

/// Subdirs that get wiped on a **major** version bump (Trigger A) or
/// on a per-conversation **flavor switch** (Trigger B).
///
/// Curated to exactly the package-manager install targets where ABI
/// mismatches across rootfs majors crash (Python wheels baked against
/// the old glibc/Python ABI, node-native modules, cargo binaries, R
/// libraries). User-generated files (`*.py`, `*.csv`, `plot.png`,
/// virtualenvs under arbitrary names, etc.) are deliberately
/// preserved.
pub const WIPE_ON_MAJOR_BUMP: &[&str] = &[
    ".local",        // pip --user, npm prefix, cargo install --root binaries
    ".cache",        // pip cache, uv cache, hf cache, build caches
    ".npm",          // npm install scratch
    ".npm-global",   // npm -g
    ".cargo",        // cargo registry + installed binaries
    ".rustup",       // rust toolchains
    ".pyenv",        // pyenv shims (if anyone installs into HOME)
    "node_modules",  // local node deps (top-level only — don't recursively walk for nested ones)
];

/// Sentinel filename dropped at the workspace root after a major-bump
/// or flavor-switch wipe. The next `execute_command` reads + unlinks
/// it and prepends a system note to the tool result.
pub const SENTINEL_ROOTFS_UPGRADED: &str = ".rootfs-upgraded";

/// Sentinel filename dropped after a per-conversation flavor-switch
/// wipe (narrower message than the rootfs-upgrade one).
pub const SENTINEL_FLAVOR_CHANGED: &str = ".flavor-changed";

/// Sentinel payload — written as JSON for forward extensibility.
/// Crate-private: only the version_manager's wipe walker + the
/// chat-extension's sentinel consumer ever touch this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WipeSentinel {
    pub old: String,
    pub new: String,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// One live mount + its inflight counters. The version manager holds
/// these in a static map keyed by `artifact_id`; the per-backend
/// `evict_artifact` calls operate against the `mount_dir`.
pub struct MountedArtifact {
    pub artifact_id: Uuid,
    pub version: String,
    pub arch: String,
    pub flavor: String,
    pub mount_dir: PathBuf,
    inflight_exec: AtomicUsize,
    inflight_mcp: AtomicUsize,
    /// Notified whenever `inflight_exec + inflight_mcp` changes.
    /// Drain tasks `notified().await` until both counters read zero.
    drained: Notify,
}

impl MountedArtifact {
    /// Live count (exec + MCP). Sequentially-consistent so a drain
    /// task that wakes on `notified()` sees the right value.
    pub fn inflight(&self) -> usize {
        self.inflight_exec.load(Ordering::SeqCst)
            + self.inflight_mcp.load(Ordering::SeqCst)
    }

    /// Per-class breakdown for the admin UI's "draining" row chip.
    pub fn inflight_breakdown(&self) -> (usize, usize) {
        (
            self.inflight_exec.load(Ordering::SeqCst),
            self.inflight_mcp.load(Ordering::SeqCst),
        )
    }
}

/// Class of usage the inflight guard represents. Tracked separately so
/// the admin UI can show "5 execs + 1 MCP server are pinning v0.1.0".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflightKind {
    Exec,
    Mcp,
}

/// RAII guard: increment on construction, decrement + notify on drop.
/// `sandbox::run_in_sandbox` holds one for the exec; `mcp_spawn`'s
/// `McpSandboxTransport` holds one for the MCP server's lifetime.
pub struct InflightGuard {
    artifact: Arc<MountedArtifact>,
    kind: InflightKind,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let counter = match self.kind {
            InflightKind::Exec => &self.artifact.inflight_exec,
            InflightKind::Mcp => &self.artifact.inflight_mcp,
        };
        // Decrement THIS class, then only notify drain waiters when
        // BOTH classes have hit zero. Audit B6: notifying on every
        // decrement made drain task spin uselessly (it'd wake, see
        // inflight() > 0, and loop). The drain loop's secondary
        // re-check (`if artifact.inflight() == 0` after notify) means
        // a spurious wake is correctness-safe but pure overhead.
        counter.fetch_sub(1, Ordering::SeqCst);
        if self.artifact.inflight() == 0 {
            self.artifact.drained.notify_waiters();
        }
    }
}

/// In-memory registry of live mounts. Keyed by artifact_id so a
/// per-conversation exec can look up its mount in O(1) and the
/// pin-swap drain task can iterate every stale-version entry.
static MOUNTED_ARTIFACTS: once_cell::sync::Lazy<
    dashmap::DashMap<Uuid, Arc<MountedArtifact>>,
> = once_cell::sync::Lazy::new(dashmap::DashMap::new);

/// Audit B2: dedup drain tasks. If `set_pin_with_drain` is called
/// twice in quick succession (rapid-fire admin clicks; two admin
/// sessions concurrently flipping the pin) we'd otherwise spawn two
/// drain tasks for the same artifact_id, both racing on
/// `evict_artifact`, `MOUNTED_ARTIFACTS.remove`, and the wipe walker.
/// The set is checked + populated atomically via `DashSet::insert` so
/// only the first caller spawns the task.
static DRAINING_ARTIFACTS: once_cell::sync::Lazy<dashmap::DashSet<Uuid>> =
    once_cell::sync::Lazy::new(dashmap::DashSet::new);

/// Register (or refresh) the in-memory tracking for an artifact that
/// was just mounted. Idempotent: a second call with the same
/// `artifact_id` returns the existing `Arc<MountedArtifact>` so
/// inflight counters carry across a re-mount.
pub fn register_mount(
    artifact_id: Uuid,
    version: &str,
    arch: &str,
    flavor: &str,
    mount_dir: PathBuf,
) -> Arc<MountedArtifact> {
    MOUNTED_ARTIFACTS
        .entry(artifact_id)
        .or_insert_with(|| {
            Arc::new(MountedArtifact {
                artifact_id,
                version: version.to_string(),
                arch: arch.to_string(),
                flavor: flavor.to_string(),
                mount_dir,
                inflight_exec: AtomicUsize::new(0),
                inflight_mcp: AtomicUsize::new(0),
                drained: Notify::new(),
            })
        })
        .clone()
}

/// Take an inflight guard against an already-registered artifact.
/// Caller MUST hold the guard for the entirety of the use (exec
/// duration / MCP transport lifetime). Returns `None` if the artifact
/// isn't in the registry — caller should treat that as "no mount yet"
/// (e.g. a stray call before `runtime_mount::ensure_rootfs_ready`).
pub fn acquire_inflight(artifact_id: Uuid, kind: InflightKind) -> Option<InflightGuard> {
    let artifact = MOUNTED_ARTIFACTS.get(&artifact_id)?.value().clone();
    let counter = match kind {
        InflightKind::Exec => &artifact.inflight_exec,
        InflightKind::Mcp => &artifact.inflight_mcp,
    };
    // Increment ONLY. The drain task waits for inflight == 0; an
    // increment cannot make that condition true, so notifying here
    // (audit B7) only causes the drain loop to wake and immediately
    // sleep again — pointless wakeup on every exec.
    counter.fetch_add(1, Ordering::SeqCst);
    Some(InflightGuard { artifact, kind })
}

/// Look up an already-registered artifact by id (used by drain tasks).
#[allow(dead_code)]
pub fn mounted_artifact(id: Uuid) -> Option<Arc<MountedArtifact>> {
    MOUNTED_ARTIFACTS.get(&id).map(|e| e.value().clone())
}

/// Snapshot of every live mount — read by the admin UI's "draining"
/// row chips. Cheap to call (clones the `Arc`s, not the structs).
pub fn list_mounted_artifacts() -> Vec<Arc<MountedArtifact>> {
    MOUNTED_ARTIFACTS.iter().map(|e| e.value().clone()).collect()
}

// NOTE (A5-01 / mount-leak): a `deregister_mounts_for_flavor` helper used to
// live here to flush MOUNTED_ARTIFACTS on wholesale flavor eviction, but its
// only intended caller — the admin DELETE `/code-sandbox/environments/{flavor}`
// path (`runtime_mount::evict_flavor`) — was retired with Plan 5 Phase 2c (see
// routes.rs). There is no flavor-wide eviction surface anymore: pin changes go
// through `set_pin_with_drain` (which removes each stale MOUNTED_ARTIFACTS entry
// on drain) and per-version deletes go through `delete_artifact`, so no path can
// leak flavor-keyed registry entries. The dead helper was removed rather than
// kept under #[allow(dead_code)].

/// Wait on the artifact's `drained` Notify until BOTH inflight
/// counters read zero. Drain tasks `await` this; in-flight execs +
/// MCP transports just need to `drop` their guards (which calls
/// `notify_waiters`) and the drain task wakes naturally.
async fn wait_until_drained(artifact: &MountedArtifact) {
    loop {
        if artifact.inflight() == 0 {
            return;
        }
        // Subscribe BEFORE the recheck so we never miss the wake.
        let waker = artifact.drained.notified();
        if artifact.inflight() == 0 {
            return;
        }
        waker.await;
    }
}

// =====================================================================
// Pin-change swap (Phase 3 high-level entry point)
// =====================================================================

/// Wipe policy chosen by `swap_policy_for_diff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapPolicy {
    /// Same major version → workspace data is preserved verbatim.
    Preserve,
    /// Different majors → wipe install-cache subdirs in every
    /// conversation- + MCP-server-workspace AFTER drain.
    WipeCachesOnDrain,
}

/// Decide whether the pin change implies a workspace install-cache
/// wipe. Same-major (minor or patch bump) → `Preserve`. Different
/// majors → `WipeCachesOnDrain`. Unparseable versions fall back to
/// `Preserve` (least-bad default — never silently nukes user state).
pub fn swap_policy_for_diff(old: &str, new: &str) -> SwapPolicy {
    let (om, _, _) = parse_semver(old);
    let (nm, _, _) = parse_semver(new);
    if om != nm {
        SwapPolicy::WipeCachesOnDrain
    } else {
        SwapPolicy::Preserve
    }
}

/// Result of a pin change. Surfaced via the `set-pin` HTTP handler so
/// the admin UI can render a "n session(s) draining" indicator.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SwapOutcome {
    pub pinned: String,
    pub was: Option<String>,
    pub draining_mounts: usize,
    pub cache_wipe: SwapPolicy,
}

/// Phase 3 entry point used by the admin handler. Wraps
/// `set_pin` with:
///   - the semver-derived workspace policy decision
///   - a spawned drain-then-evict task per stale-version mount
///   - the per-server / per-conversation install-cache wipe (on
///     major bump, after drain)
///
/// `workspace_root` is the same value `CodeSandboxState.workspace_root`
/// holds; passed in explicitly so this function is straightforward to
/// unit-test against a temp dir.
pub async fn set_pin_with_drain(
    pool: &PgPool,
    target_version: &str,
    workspace_root: PathBuf,
) -> Result<SwapOutcome, VersionError> {
    let old = current_pin(pool).await?;
    if old.as_deref() == Some(target_version) {
        return Ok(SwapOutcome {
            pinned: target_version.to_string(),
            was: old,
            draining_mounts: 0,
            cache_wipe: SwapPolicy::Preserve,
        });
    }

    set_pin(pool, target_version).await?;

    let policy = match old.as_deref() {
        Some(o) => swap_policy_for_diff(o, target_version),
        None => SwapPolicy::Preserve,
    };

    // Pick out every live mount that no longer matches the new pin.
    let draining: Vec<Arc<MountedArtifact>> = MOUNTED_ARTIFACTS
        .iter()
        .map(|e| e.value().clone())
        .filter(|m| m.version != target_version)
        .collect();
    let draining_count = draining.len();

    for stale in draining {
        // Audit B2: skip if a drain task for this artifact_id is
        // already in flight. `DashSet::insert` returns false when
        // the value was already present, so only the first caller
        // spawns the task.
        if !DRAINING_ARTIFACTS.insert(stale.artifact_id) {
            tracing::debug!(
                artifact_id = %stale.artifact_id,
                "code_sandbox: drain task already in flight; skipping dup"
            );
            continue;
        }
        let old_v = old.clone();
        let new_v = target_version.to_string();
        let workspace_root = workspace_root.clone();
        tokio::spawn(async move {
            // Guard so the dedup marker is always cleared, even on
            // panic between wait_until_drained and the wipe walker.
            struct DrainGuard(Uuid);
            impl Drop for DrainGuard {
                fn drop(&mut self) {
                    DRAINING_ARTIFACTS.remove(&self.0);
                }
            }
            let _drain_guard = DrainGuard(stale.artifact_id);

            tracing::info!(
                artifact_id = %stale.artifact_id,
                version = %stale.version,
                "code_sandbox: drain task waiting on inflight counters"
            );
            wait_until_drained(&stale).await;
            tracing::info!(
                artifact_id = %stale.artifact_id,
                "code_sandbox: drained; evicting"
            );

            // Evict the mount via the platform backend. Best-effort:
            // a failure (e.g. fusermount returned non-zero because
            // another process held a stale open FD) is logged but
            // doesn't block the wipe below.
            let evict = crate::modules::code_sandbox::backend::active()
                .evict_artifact(&stale.mount_dir, &stale.flavor, &stale.version)
                .await;
            tracing::info!(
                artifact_id = %stale.artifact_id,
                evicted = evict.was_cached,
                bytes_freed = evict.bytes_freed,
                "code_sandbox: evict_artifact returned"
            );
            MOUNTED_ARTIFACTS.remove(&stale.artifact_id);

            if policy == SwapPolicy::WipeCachesOnDrain {
                let sentinel = WipeSentinel {
                    old: old_v.clone().unwrap_or_default(),
                    new: new_v.clone(),
                    at: chrono::Utc::now(),
                };
                let result = wipe_install_caches_in_root(&workspace_root, &sentinel);
                tracing::info!(
                    conversation_dirs = result.conversation_dirs,
                    mcp_server_dirs = result.mcp_server_dirs,
                    subdirs_removed = result.subdirs_removed,
                    "workspace_cleanup: major-bump wipe complete"
                );
            }
        });
    }

    Ok(SwapOutcome {
        pinned: target_version.to_string(),
        was: old,
        draining_mounts: draining_count,
        cache_wipe: policy,
    })
}

/// What the wipe walker did. Surfaced via the tracing log for
/// post-hoc admin visibility (the actual per-path detail is too
/// noisy for a single log line). Crate-private: callers outside
/// the version_manager only care about the structured tracing
/// fields, not the strict-type.
#[derive(Debug, Default, Clone)]
pub(crate) struct WipeResult {
    pub conversation_dirs: usize,
    pub mcp_server_dirs: usize,
    pub subdirs_removed: usize,
}

/// Walk a workspace_root and `rm -rf` the curated install-cache
/// subdirs inside every per-conversation and per-MCP-server workspace
/// directory. Drops a `.rootfs-upgraded` sentinel in each affected
/// workspace so the next `execute_command` (or next MCP tool call)
/// can prepend a system note to its tool result.
///
/// Skips `attachments/` and `identity/` (shared-state dirs that are
/// neither per-conversation nor per-MCP-server).
pub(crate) fn wipe_install_caches_in_root(
    workspace_root: &std::path::Path,
    sentinel: &WipeSentinel,
) -> WipeResult {
    let mut result = WipeResult::default();
    if !workspace_root.is_dir() {
        return result;
    }
    let sentinel_json = serde_json::to_string(sentinel).unwrap_or_default();

    // Layer 1: per-conversation dirs (children of workspace_root) +
    //          the `mcp/` subtree.
    let entries = match std::fs::read_dir(workspace_root) {
        Ok(e) => e,
        Err(_) => return result,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Reject symlinks at the workspace_root level — an operator (or
        // attacker with workspace-write) could plant `<wr>/00000000-...
        // -000evil` as a symlink to `/etc` and the walker would
        // recurse + wipe inside the symlink target. Audit B13/B14.
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Layer 2: MCP per-server dirs under `<workspace_root>/mcp/`.
        if name == "mcp" {
            let mcp_dirs = match std::fs::read_dir(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for mcp_entry in mcp_dirs.flatten() {
                let mcp_path = mcp_entry.path();
                let mcp_meta = match std::fs::symlink_metadata(&mcp_path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if mcp_meta.file_type().is_symlink() || !mcp_meta.is_dir() {
                    continue;
                }
                // Require the MCP server dir name to parse as a Uuid
                // — server IDs are deterministic v5 / v4 Uuids, and
                // anything else is operator-created garbage we should
                // not recurse into.
                let mcp_name = match mcp_path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                if Uuid::parse_str(mcp_name).is_err() {
                    continue;
                }
                let n = wipe_subdirs_in(&mcp_path, &sentinel_json);
                result.mcp_server_dirs += 1;
                result.subdirs_removed += n;
            }
            continue;
        }

        // Skip shared subsystem dirs (not per-conversation):
        //   `attachments/` is shared staging for bind-mounted user
        //   attachments; `identity/` is the shared synthetic
        //   passwd/group.
        if name == "attachments" || name == "identity" {
            continue;
        }

        // Per-conversation dir names MUST be valid Uuids. Audit B14:
        // without this an operator-planted `<wr>/etc-symlink-target`
        // would be treated as a conv dir and recursed into.
        if Uuid::parse_str(name).is_err() {
            continue;
        }

        let n = wipe_subdirs_in(&path, &sentinel_json);
        result.conversation_dirs += 1;
        result.subdirs_removed += n;
    }
    result
}

/// Per-workspace wipe primitive: `rm -rf` each subdir in
/// `WIPE_ON_MAJOR_BUMP` that exists, then drop a `.rootfs-upgraded`
/// sentinel. Returns the count of subdirs that were actually removed.
fn wipe_subdirs_in(workspace_dir: &std::path::Path, sentinel_json: &str) -> usize {
    let mut removed = 0;
    for sub in WIPE_ON_MAJOR_BUMP {
        let target = workspace_dir.join(sub);
        match std::fs::symlink_metadata(&target) {
            Ok(_) => {
                let r = if target.is_dir() {
                    std::fs::remove_dir_all(&target)
                } else {
                    std::fs::remove_file(&target)
                };
                if r.is_ok() {
                    removed += 1;
                } else if let Err(e) = r {
                    tracing::warn!(
                        path = %target.display(),
                        "workspace_cleanup: failed to remove {sub}: {e}"
                    );
                }
            }
            Err(_) => continue, // missing — fine
        }
    }
    // Drop the sentinel (best-effort).
    let sentinel_path = workspace_dir.join(SENTINEL_ROOTFS_UPGRADED);
    if let Err(e) = std::fs::write(&sentinel_path, sentinel_json) {
        tracing::warn!(
            path = %sentinel_path.display(),
            "workspace_cleanup: failed to drop sentinel: {e}"
        );
    }
    removed
}

/// Per-conversation flavor-switch wipe (Trigger B). Called
/// synchronously from `tools/execute.rs` when the LLM changes the
/// flavor mid-conversation. Wipes only THIS one workspace dir's
/// install-cache subdirs and drops a `.flavor-changed` sentinel.
pub fn wipe_install_caches_for_conversation(
    workspace_dir: &std::path::Path,
    old_flavor: &str,
    new_flavor: &str,
) -> WipeResult {
    let mut result = WipeResult::default();
    if !workspace_dir.is_dir() {
        return result;
    }
    let sentinel = WipeSentinel {
        old: old_flavor.to_string(),
        new: new_flavor.to_string(),
        at: chrono::Utc::now(),
    };
    let sentinel_json = serde_json::to_string(&sentinel).unwrap_or_default();
    let n = wipe_subdirs_in(workspace_dir, &sentinel_json);
    // Overwrite the sentinel name to the flavor-specific one (the
    // helper drops a `.rootfs-upgraded`; rename to
    // `.flavor-changed` for this trigger).
    let _ = std::fs::rename(
        workspace_dir.join(SENTINEL_ROOTFS_UPGRADED),
        workspace_dir.join(SENTINEL_FLAVOR_CHANGED),
    );
    result.conversation_dirs = 1;
    result.subdirs_removed = n;
    result
}

/// Read + unlink the most-recent wipe sentinel in `workspace_dir`,
/// formatted as a human-readable system-note string suitable for
/// prepending to the tool result. Returns `None` if no sentinel is
/// present.
///
/// Looks for `.rootfs-upgraded` first (major-bump), then
/// `.flavor-changed` (per-conversation flavor switch). Both are
/// removed after reading so the next call doesn't re-prepend the
/// same message.
pub fn consume_workspace_sentinel(workspace_dir: &std::path::Path) -> Option<String> {
    for (filename, is_major) in [
        (SENTINEL_ROOTFS_UPGRADED, true),
        (SENTINEL_FLAVOR_CHANGED, false),
    ] {
        let path = workspace_dir.join(filename);
        let body = match std::fs::read_to_string(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let _ = std::fs::remove_file(&path);
        let sentinel: WipeSentinel = match serde_json::from_str(&body) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let msg = if is_major {
            format!(
                "Sandbox upgraded from v{} to v{} (major bump). \
                 Package-manager caches (.local, .cache, .npm, ...) were cleared; \
                 reinstall (pip / npm / ...) anything you need. \
                 Your files in /workspace are intact.",
                sentinel.old, sentinel.new
            )
        } else {
            format!(
                "Sandbox flavor changed from {} to {} in this conversation. \
                 Package-manager caches were cleared; reinstall anything you need. \
                 Your files in /workspace are intact.",
                sentinel.old, sentinel.new
            )
        };
        return Some(msg);
    }
    None
}

// =====================================================================
// Tier 1 unit tests
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_degraded_shape() {
        // The GitHub catalog is passed through; everything DB/state-derived is
        // empty, and the reason is carried verbatim.
        let releases = vec![RootfsRelease {
            version: "0.0.6-alpha".to_string(),
            published_at: None,
            draft: false,
            prerelease: true,
            asset_names: vec!["ziee-sandbox-rootfs-x86_64-full.squashfs".to_string()],
        }];
        let s = build_degraded(SandboxAvailability::DisabledInConfig, releases.clone());
        assert_eq!(s.availability, SandboxAvailability::DisabledInConfig);
        assert_eq!(s.available.len(), 1);
        assert_eq!(s.available[0].version, "0.0.6-alpha");
        assert!(s.installed.is_empty());
        assert!(s.draining.is_empty());
        assert_eq!(s.pinned_version, None);
        assert_eq!(s.conversation_count, 0);
        assert_eq!(s.mcp_server_workspace_count, 0);
        // Authoritative host fields are still populated so the UI can offer the
        // artifact this machine would run once enabled.
        assert!(!s.host_arch.is_empty());
        assert!(!s.host_package.is_empty());

        // An empty catalog (GitHub unreachable) still yields a valid degraded
        // snapshot carrying the reason.
        let empty = build_degraded(SandboxAvailability::HostUnsupported, Vec::new());
        assert_eq!(empty.availability, SandboxAvailability::HostUnsupported);
        assert!(empty.available.is_empty());
    }

    #[test]
    fn semver_tag_regex_accepts_valid() {
        for t in [
            "v0.0.1",
            "v0.1.0",
            "v1.0.0",
            "v10.20.30",
            // Audit B9: prerelease tags must validate.
            "v0.0.2-alpha",
            "v1.2.3-rc.1",
            "v0.1.0-rc1",
            "v0.1.0-alpha.0",
        ] {
            assert!(is_valid_semver_tag(t), "should accept {t}");
        }
    }

    #[test]
    fn semver_tag_regex_rejects_invalid() {
        for t in [
            "0.1.0",                        // missing v
            "v0.1",                         // 2 components
            "v0.1.0+meta",                  // build metadata (we don't accept)
            "v0.01.0",                      // leading zero on minor — audit B9
            "v01.0.0",                      // leading zero on major
            "v1.2.3-",                      // empty prerelease
            "v1.2.3-rc..1",                 // empty prerelease identifier
            "v1.2.3-rc.01",                 // numeric prerelease id with leading zero
            "v1.2.3-rc/1",                  // invalid char in prerelease id
            "vfoo",                         // non-numeric
            "sandbox-rootfs-v1.r0-x86_64",  // legacy tag shape
        ] {
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
    fn swap_policy_for_diff_picks_wipe_on_major_bump() {
        assert_eq!(swap_policy_for_diff("0.1.0", "0.1.1"), SwapPolicy::Preserve);
        assert_eq!(swap_policy_for_diff("0.1.0", "0.2.0"), SwapPolicy::Preserve);
        assert_eq!(swap_policy_for_diff("0.9.9", "0.10.0"), SwapPolicy::Preserve);
        assert_eq!(
            swap_policy_for_diff("0.1.0", "1.0.0"),
            SwapPolicy::WipeCachesOnDrain
        );
        assert_eq!(
            swap_policy_for_diff("1.2.3", "2.0.0"),
            SwapPolicy::WipeCachesOnDrain
        );
        // Same version is a no-op (Preserve covers it; callers
        // short-circuit before this is reached).
        assert_eq!(swap_policy_for_diff("0.1.0", "0.1.0"), SwapPolicy::Preserve);
        // `parse_semver` is lenient: an unparseable component parses to 0.
        // So inputs whose major both resolve to 0 compare equal → Preserve
        // (we never wipe just because a version string was malformed and
        // both sides land on the same major). Pins are validated semver in
        // practice, so these are defensive cases.
        assert_eq!(swap_policy_for_diff("0.1.0", "not-a-version"), SwapPolicy::Preserve);
        assert_eq!(swap_policy_for_diff("garbage", "also-garbage"), SwapPolicy::Preserve);
        // The policy keys off "majors differ", not direction or "major == 0":
        // a major DOWNGRADE wipes too, and an equal NON-ZERO major preserves.
        assert_eq!(
            swap_policy_for_diff("2.0.0", "1.5.0"),
            SwapPolicy::WipeCachesOnDrain
        );
        assert_eq!(swap_policy_for_diff("2.3.0", "2.9.0"), SwapPolicy::Preserve);
    }

    #[test]
    fn wipe_walker_drops_install_caches_and_keeps_user_files() {
        let workspace_root = tempfile::tempdir().unwrap();
        let conv_a = workspace_root.path().join("00000000-0000-0000-0000-00000000000a");
        let conv_b = workspace_root.path().join("00000000-0000-0000-0000-00000000000b");
        std::fs::create_dir_all(conv_a.join(".local")).unwrap();
        std::fs::create_dir_all(conv_a.join(".cache/pip")).unwrap();
        std::fs::write(conv_a.join("notes.md"), "user file").unwrap();
        std::fs::write(conv_a.join("output.csv"), "x,y\n").unwrap();
        std::fs::create_dir_all(conv_b.join(".npm")).unwrap();
        std::fs::write(conv_b.join("plot.png"), b"PNG").unwrap();
        // Shared subsystem dirs the walker must skip.
        std::fs::create_dir_all(workspace_root.path().join("attachments")).unwrap();
        std::fs::create_dir_all(workspace_root.path().join("identity")).unwrap();
        // Per-MCP-server workspace.
        let mcp_server =
            workspace_root.path().join("mcp").join("11111111-1111-1111-1111-111111111111");
        std::fs::create_dir_all(mcp_server.join(".local/lib")).unwrap();
        std::fs::write(mcp_server.join("server-state.json"), "{}").unwrap();

        let sentinel = WipeSentinel {
            old: "0.9.0".to_string(),
            new: "1.0.0".to_string(),
            at: chrono::Utc::now(),
        };
        let result = wipe_install_caches_in_root(workspace_root.path(), &sentinel);

        // Counts.
        assert_eq!(result.conversation_dirs, 2);
        assert_eq!(result.mcp_server_dirs, 1);
        assert!(result.subdirs_removed >= 3); // .local, .cache, .npm (.local from mcp)

        // Conversation workspaces — install caches gone, user files intact.
        assert!(!conv_a.join(".local").exists());
        assert!(!conv_a.join(".cache").exists());
        assert!(conv_a.join("notes.md").exists());
        assert!(conv_a.join("output.csv").exists());
        assert!(!conv_b.join(".npm").exists());
        assert!(conv_b.join("plot.png").exists());

        // MCP server workspace.
        assert!(!mcp_server.join(".local").exists());
        assert!(mcp_server.join("server-state.json").exists());

        // Sentinels dropped.
        assert!(conv_a.join(SENTINEL_ROOTFS_UPGRADED).exists());
        assert!(conv_b.join(SENTINEL_ROOTFS_UPGRADED).exists());
        assert!(mcp_server.join(SENTINEL_ROOTFS_UPGRADED).exists());
    }

    #[test]
    fn flavor_switch_wipes_only_caller_conversation() {
        let workspace_root = tempfile::tempdir().unwrap();
        let conv_a = workspace_root.path().join("00000000-0000-0000-0000-00000000000a");
        let conv_b = workspace_root.path().join("00000000-0000-0000-0000-00000000000b");
        std::fs::create_dir_all(conv_a.join(".local/lib")).unwrap();
        std::fs::create_dir_all(conv_b.join(".local/lib")).unwrap();

        let result = wipe_install_caches_for_conversation(&conv_a, "minimal", "full");
        assert_eq!(result.conversation_dirs, 1);
        assert!(result.subdirs_removed >= 1);

        // A wiped, B untouched.
        assert!(!conv_a.join(".local").exists());
        assert!(conv_b.join(".local/lib").exists());

        // Sentinel uses the flavor-changed name.
        assert!(conv_a.join(SENTINEL_FLAVOR_CHANGED).exists());
        assert!(!conv_a.join(SENTINEL_ROOTFS_UPGRADED).exists());
    }

    #[test]
    fn consume_workspace_sentinel_reads_unlinks_returns_message() {
        let dir = tempfile::tempdir().unwrap();
        let sentinel = WipeSentinel {
            old: "0.1.0".to_string(),
            new: "1.0.0".to_string(),
            at: chrono::Utc::now(),
        };
        let json_text = serde_json::to_string(&sentinel).unwrap();
        std::fs::write(dir.path().join(SENTINEL_ROOTFS_UPGRADED), &json_text).unwrap();

        let note = consume_workspace_sentinel(dir.path()).expect("sentinel present");
        assert!(note.contains("v0.1.0"));
        assert!(note.contains("v1.0.0"));
        assert!(note.contains("major bump"));
        assert!(!dir.path().join(SENTINEL_ROOTFS_UPGRADED).exists());

        // Second call: sentinel unlinked, no message.
        assert!(consume_workspace_sentinel(dir.path()).is_none());
    }

    #[test]
    fn consume_workspace_sentinel_handles_flavor_switch() {
        let dir = tempfile::tempdir().unwrap();
        let sentinel = WipeSentinel {
            old: "minimal".to_string(),
            new: "full".to_string(),
            at: chrono::Utc::now(),
        };
        let json_text = serde_json::to_string(&sentinel).unwrap();
        std::fs::write(dir.path().join(SENTINEL_FLAVOR_CHANGED), &json_text).unwrap();

        let note = consume_workspace_sentinel(dir.path()).expect("sentinel present");
        assert!(note.contains("minimal"));
        assert!(note.contains("full"));
        assert!(note.contains("flavor"));
        assert!(!dir.path().join(SENTINEL_FLAVOR_CHANGED).exists());
    }

    #[test]
    fn inflight_guard_round_trip() {
        let id = Uuid::new_v4();
        let _registry_guard =
            register_mount(id, "0.1.0", "x86_64", "minimal", std::path::PathBuf::from("/tmp"));
        let artifact = mounted_artifact(id).unwrap();
        assert_eq!(artifact.inflight(), 0);

        let exec = acquire_inflight(id, InflightKind::Exec).unwrap();
        assert_eq!(artifact.inflight(), 1);
        let mcp = acquire_inflight(id, InflightKind::Mcp).unwrap();
        assert_eq!(artifact.inflight(), 2);
        assert_eq!(artifact.inflight_breakdown(), (1, 1));

        drop(exec);
        assert_eq!(artifact.inflight(), 1);
        drop(mcp);
        assert_eq!(artifact.inflight(), 0);

        // Cleanup so a parallel test on this registry doesn't see leftover.
        MOUNTED_ARTIFACTS.remove(&id);
    }

    #[test]
    fn build_download_url_uses_default_and_mirror_envs() {
        // The CODE_SANDBOX_ROOTFS_MIRROR override is `#[cfg(debug_assertions)]`
        // (see `build_download_url`), so this test — always a debug build —
        // exercises the mirror path; a release binary compiles the env read
        // out entirely and always points at GitHub. `set_var`/`remove_var`
        // are unsafe in edition 2024; only this unit test reads the var in a
        // `--lib` run, so set→assert→remove is contamination-safe.

        // 1. Default path — no env set → real GitHub Releases host.
        unsafe { std::env::remove_var("CODE_SANDBOX_ROOTFS_MIRROR") };
        let url = build_download_url("v0.1.0", "x.squashfs");
        assert!(
            url.contains("ziee-ai/sandbox-rootfs/releases/download")
                || url.contains(ROOTFS_REPO),
            "unexpected default URL: {url}"
        );
        assert!(url.ends_with("/v0.1.0/x.squashfs"));
        assert!(
            url.starts_with("https://github.com/"),
            "default URL must point at GitHub: {url}"
        );

        // 2. Mirror set (debug builds only) → loopback host, trailing slash trimmed.
        unsafe { std::env::set_var("CODE_SANDBOX_ROOTFS_MIRROR", "http://127.0.0.1:9999/m/") };
        let mirrored = build_download_url("v0.1.0", "x.squashfs");
        assert_eq!(mirrored, "http://127.0.0.1:9999/m/v0.1.0/x.squashfs");

        // 3. Empty mirror falls back to the default GitHub host.
        unsafe { std::env::set_var("CODE_SANDBOX_ROOTFS_MIRROR", "") };
        let empty = build_download_url("v0.1.0", "x.squashfs");
        assert!(
            empty.starts_with("https://github.com/"),
            "empty mirror must fall back to GitHub: {empty}"
        );

        // Cleanup so sibling tests never observe the override.
        unsafe { std::env::remove_var("CODE_SANDBOX_ROOTFS_MIRROR") };
    }

    // audit id all-3fa5d25af4f3 — rootfs download FAILURE handling. A cached
    // artifact whose bytes don't match the recorded sha256 (a truncated /
    // tampered / hijacked-mirror download) must be REJECTED so the caller
    // re-fetches; a missing file must likewise read as not-valid. The existing
    // runtime_fetch tests cover only locking; nothing exercised the sha256
    // verification verdict that gates a corrupt download.
    fn artifact_with(path: &std::path::Path, sha256: &str) -> RootfsArtifact {
        RootfsArtifact {
            id: uuid::Uuid::new_v4(),
            version: "0.0.1".into(),
            arch: "x86_64".into(),
            flavor: "minimal".into(),
            package: "squashfs".into(),
            sha256: sha256.to_string(),
            artifact_path: path.to_string_lossy().into_owned(),
            cosign_bundle: None,
            status: "ready".into(),
            downloaded_at: chrono::Utc::now(),
            last_used_at: None,
        }
    }

    #[tokio::test]
    async fn verify_cached_sha256_rejects_mismatch_and_missing() {
        let dir = std::env::temp_dir().join(format!("ziee-rootfs-sha-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("artifact.squashfs");
        std::fs::write(&file, b"the real rootfs bytes").unwrap();

        // The artifact's TRUE sha256 (computed by the same primitive the
        // download path uses) → verification passes.
        let real = sha256_file(&file).unwrap();
        let ok = artifact_with(&file, &real);
        assert!(
            verify_cached_sha256(&ok).await.unwrap(),
            "a byte-for-byte cached artifact must verify"
        );
        // Case-insensitive hex comparison is accepted.
        let ok_upper = artifact_with(&file, &real.to_uppercase());
        assert!(
            verify_cached_sha256(&ok_upper).await.unwrap(),
            "sha256 comparison must be case-insensitive"
        );

        // A wrong sha256 (corrupt / tampered download) → rejected.
        let bad = artifact_with(
            &file,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(
            !verify_cached_sha256(&bad).await.unwrap(),
            "a sha256 mismatch must be rejected so the caller re-downloads"
        );

        // A missing artifact file → not valid (re-download).
        let gone = artifact_with(&dir.join("nope.squashfs"), &real);
        assert!(
            !verify_cached_sha256(&gone).await.unwrap(),
            "a missing cached file must read as not-valid"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
