//! HubManager — GitHub-Releases-backed catalog of models, assistants, MCP servers.
//!
//! Source of truth: `ziee-ai/hub` repo, tagged + signed releases. At
//! `tag → release.yml → ziee-ai/hub` produces `hub.tar.gz` (flat bundle of
//! manifests + schemas + index) plus `hub.index.json`, each with a
//! `.sha256` and a keyless cosign `.cosign.bundle` sidecar.
//!
//! On boot the server installs an embedded seed catalog (compiled via
//! `include_dir!` from `binaries/hub-seed/`, which `build_helper/hub_seed.rs`
//! populates at build time from the latest ziee-ai/hub release —
//! verified with the same sha256 + cosign chain the runtime refresh
//! uses) so the hub UI renders read-only even when GitHub is
//! unreachable post-install. `SEED_HUB_VERSION` is set by the build
//! helper from the resolved tag; keeping the seed + version in
//! lockstep is the build's responsibility, not the maintainer's.
//!
//! Refresh path: download both files into a staging dir, sha256-check
//! both, sigstore-verify both against the expected keyless OIDC
//! identity, unpack the tarball, atomically rotate
//! `<app_data>/hub/current/` to point at the new version. Failure at any
//! step leaves the previous `current/` untouched.

use chrono::{DateTime, Utc};
use include_dir::{Dir, include_dir};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::models::{HubAssistant, HubCategory, HubData, HubMCPServer, HubModel};
use crate::common::AppError;

// =====================================================================
// Configuration
// =====================================================================

pub const HUB_REPO_OWNER: &str = "ziee-ai";
pub const HUB_REPO_NAME: &str = "hub";

/// Cosign keyless OIDC identity that release.yml signs as. Verified
/// in-process via the sigstore crate — see `verify_cosign_bundle`.
pub fn cosign_expected_identity(tag: &str) -> String {
    format!(
        "https://github.com/{}/{}/.github/workflows/release.yml@refs/tags/{}",
        HUB_REPO_OWNER, HUB_REPO_NAME, tag
    )
}

pub const COSIGN_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

// =====================================================================
// Dev/test overrides — physically compiled OUT of release builds via
// `cfg!(debug_assertions)`, mirroring code_sandbox's dev-mirror pattern.
// They let the integration suite point the fetcher at a local mock
// release server (no network, no real cosign signature) without any
// risk of the mechanism being reachable in a shipped binary.
// =====================================================================

/// Base for the GitHub REST API (releases list). Override in debug via
/// `ZIEE_HUB_API_BASE_OVERRIDE` (e.g. `http://127.0.0.1:PORT`).
fn hub_api_base() -> String {
    if cfg!(debug_assertions)
        && let Ok(v) = std::env::var("ZIEE_HUB_API_BASE_OVERRIDE")
        && !v.is_empty()
    {
        return v;
    }
    "https://api.github.com".to_string()
}

/// Base for release asset downloads. Override in debug via
/// `ZIEE_HUB_DOWNLOAD_BASE_OVERRIDE`.
fn hub_download_base() -> String {
    if cfg!(debug_assertions)
        && let Ok(v) = std::env::var("ZIEE_HUB_DOWNLOAD_BASE_OVERRIDE")
        && !v.is_empty()
    {
        return v;
    }
    "https://github.com".to_string()
}

/// On-disk root of the hub catalog (`current/`, `.staging/`,
/// `.previous/`). Debug builds honor `ZIEE_HUB_DATA_DIR_OVERRIDE` so
/// the integration suite can give each test an isolated catalog dir —
/// the catalog is per-deployment global mutable state, so without
/// isolation a mutating test (refresh/activate) contaminates every
/// other test sharing the same app_data dir. Always `app_data/hub` in
/// release.
fn hub_root_for(app_data: &Path) -> PathBuf {
    if cfg!(debug_assertions)
        && let Ok(d) = std::env::var("ZIEE_HUB_DATA_DIR_OVERRIDE")
        && !d.is_empty()
    {
        return PathBuf::from(d);
    }
    app_data.join("hub")
}

/// When set in a debug build, the cosign keyless verification step is
/// skipped (the mock release server can't mint a real Sigstore bundle).
/// Always false in release — there is no way to disable cosign in a
/// shipped binary.
fn allow_unsigned() -> bool {
    cfg!(debug_assertions)
        && std::env::var("ZIEE_HUB_ALLOW_UNSIGNED")
            .map(|v| v == "1")
            .unwrap_or(false)
}

/// Hard cap on any single hub artifact. The bundle is ~10 KB at v0.0.1
/// (13 manifests) and grows linearly with catalog size; 32 MiB leaves
/// headroom for thousands of items while preventing an upstream
/// redirect from filling memory via `.bytes().await`.
const MAX_HUB_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;

/// Server semver. The `compat()` helper compares this against each
/// IndexItem's `min_ziee_version` to partition the catalog into
/// installable vs incompatible.
pub fn server_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// =====================================================================
// Embedded seed (build-time fetched from ziee-ai/hub releases)
// =====================================================================
//
// `build_helper/hub_seed.rs` runs at compile time, downloads the
// latest non-prerelease tag from github.com/ziee-ai/hub (or honors
// `HUB_RELEASE_TAG` for pinned builds), sha256 + cosign verifies,
// and stages the catalog into `binaries/hub-seed/`. The macro below
// then bakes that staged directory into the binary. The build fails
// loudly if the fetch fails — offline / air-gapped operators must
// pin a tag + manually stage the dir before building.

static HUB_SEED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/binaries/hub-seed");

/// Seed catalog version. Written by `build_helper/hub_seed.rs` from
/// the resolved tag (with the leading `v` stripped) to keep this
/// const in lockstep with whatever `binaries/hub-seed/` actually
/// contains. The pre-build-fetch-era code hardcoded this string and
/// drifted whenever someone forgot to bump it.
pub const SEED_HUB_VERSION: &str =
    include_str!(concat!(env!("OUT_DIR"), "/hub_seed_version.txt"));

/// Marker file dropped into `current/` when the active catalog is the
/// embedded seed (never fetched + verified from GitHub). A successful
/// refresh rotates in a fresh dir that lacks it.
const SEED_MARKER: &str = ".seed";

/// Process-wide lock serializing catalog refresh/activate so concurrent
/// callers don't clobber the shared `.staging` / `.previous` dirs.
static HUB_REFRESH_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

/// Origin of the active on-disk catalog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CatalogProvenance {
    /// Embedded seed (boot fallback / air-gapped). Trusted (compiled
    /// into the binary) and installable, but not a live fetch.
    Seed,
    /// Downloaded + sha256 + cosign-verified from ziee-ai/hub.
    Github,
}

// =====================================================================
// Catalog types (returned from `catalog()` / consumed by handlers)
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Catalog {
    pub schema_version: u32,
    pub hub_version: String,
    #[serde(default)]
    pub generated_at: Option<String>,
    pub items: Vec<IndexItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexItem {
    pub id: String,
    pub category: HubCategory,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    pub added_at: Option<String>,
    pub min_ziee_version: Option<String>,
    pub manifest_path: String,
}

/// Full manifest for one hub item, returned by `GET /api/hub/manifest/:id`.
///
/// A struct (not a `#[serde(tag)]` enum) because the tagged-enum +
/// `Box<Struct>` form produces an empty OpenAPI schema — clients
/// couldn't see the payload fields. Exactly one of `model` /
/// `assistant` / `mcp_server` is populated, matching `category`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubManifest {
    pub category: HubCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<HubModel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant: Option<Box<HubAssistant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_server: Option<Box<HubMCPServer>>,
}

impl HubManifest {
    fn model(m: HubModel) -> Self {
        Self {
            category: HubCategory::Model,
            model: Some(Box::new(m)),
            assistant: None,
            mcp_server: None,
        }
    }
    fn assistant(a: HubAssistant) -> Self {
        Self {
            category: HubCategory::Assistant,
            model: None,
            assistant: Some(Box::new(a)),
            mcp_server: None,
        }
    }
    fn mcp_server(s: HubMCPServer) -> Self {
        Self {
            category: HubCategory::McpServer,
            model: None,
            assistant: None,
            mcp_server: Some(Box::new(s)),
        }
    }
}

/// Result of a `compat(item)` check.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Compat {
    /// No `min_ziee_version` set, or server version >= required.
    Ok,
    /// Server version is older than the manifest's `min_ziee_version`.
    TooOld { required: String },
}

impl Compat {
    pub fn is_ok(&self) -> bool {
        matches!(self, Compat::Ok)
    }
}

/// Returned from `refresh()` so handlers can surface "no change" vs
/// "advanced to v0.X.Y" in toasts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefreshOutcome {
    pub previous_version: Option<String>,
    pub new_version: String,
    pub updated: bool,
    pub cosign_verified: bool,
    pub refreshed_at: DateTime<Utc>,
}

/// One published catalog version on GitHub Releases. Surfaced by
/// `list_releases()` for the admin version picker.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubReleaseInfo {
    /// Version without the leading `v` (e.g. `0.0.2-alpha`).
    pub version: String,
    /// Full git tag (e.g. `v0.0.2-alpha`).
    pub tag: String,
    pub prerelease: bool,
    pub published_at: Option<String>,
}

// =====================================================================
// HubManager
// =====================================================================

pub struct HubManager {
    app_data_dir: PathBuf,
}

impl HubManager {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Result<Self, AppError> {
        Ok(Self {
            app_data_dir: app_data_dir.into(),
        })
    }

    fn hub_root(&self) -> PathBuf {
        hub_root_for(&self.app_data_dir)
    }

    fn current_dir(&self) -> PathBuf {
        self.hub_root().join("current")
    }

    fn staging_dir(&self) -> PathBuf {
        self.hub_root().join(".staging")
    }

    /// On boot: install the embedded seed catalog into
    /// `<app_data>/hub/current/` if it doesn't already exist.
    ///
    /// Also UPGRADES a stale seed-provenance cache when the binary
    /// embeds a newer `SEED_HUB_VERSION`. Without this, a long-lived
    /// data-dir keeps serving the catalog version it was first seeded
    /// with, even after the user upgrades the binary — admins see
    /// "installed v2, current v3" in the Updates tab and Re-install
    /// silently re-stamps the row with the SAME stale catalog version
    /// (the cache the runtime is reading from), so the row never
    /// drops off the Updates list.
    ///
    /// A GitHub-fetched cache (`provenance == Github`) is NEVER
    /// auto-replaced — it's been cosign-verified and may legitimately
    /// be newer than the embedded seed (the seed is bumped on the
    /// release-engineering cadence; a fetch is on the operator's
    /// schedule). Operators who want to roll a verified cache forward
    /// or back go through `/api/hub/refresh` or `/api/hub/activate`.
    pub fn initialize(&self) -> Result<(), AppError> {
        let current = self.current_dir();
        let index_path = current.join("index.json");

        if !index_path.exists() {
            // Fresh install — fall through to install the seed.
        } else {
            // Cache already exists. Replace it ONLY when:
            //   1. it was seeded by a prior boot (has `.seed` marker), AND
            //   2. the embedded `SEED_HUB_VERSION` is strictly newer
            //      than the cached `hub_version` (semver compare).
            // Anything else (GitHub-fetched cache, same-version seed,
            // unreadable / malformed cache) is left alone.
            let is_seed = current.join(SEED_MARKER).exists();
            if !is_seed {
                // GitHub-fetched cache — operator's source of truth.
                return Ok(());
            }
            let cached_version = match fs::read(&index_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Catalog>(&bytes).ok())
            {
                Some(cat) => cat.hub_version,
                None => {
                    // Marker says seed but the index is unreadable —
                    // treat as a corrupt cache and reseed.
                    tracing::warn!(
                        "hub: seed cache at {} is unreadable; reseeding",
                        index_path.display()
                    );
                    Self::overwrite_with_seed(&current)?;
                    return Ok(());
                }
            };

            // Semver compare; trim the seed-version trailing newline
            // (`include_str!` keeps the file's terminating `\n`).
            let seed_ver_str = SEED_HUB_VERSION.trim();
            let cached_ver = match semver::Version::parse(&cached_version) {
                Ok(v) => v,
                Err(_) => {
                    tracing::warn!(
                        "hub: cached hub_version '{}' is not valid semver; leaving cache as-is",
                        cached_version
                    );
                    return Ok(());
                }
            };
            let seed_ver = match semver::Version::parse(seed_ver_str) {
                Ok(v) => v,
                Err(e) => {
                    // Build-time invariant guarantees SEED_HUB_VERSION
                    // is semver — log + bail rather than corrupting the
                    // cache.
                    tracing::error!(
                        "hub: embedded SEED_HUB_VERSION '{}' is not valid semver: {} — leaving cache as-is",
                        seed_ver_str,
                        e
                    );
                    return Ok(());
                }
            };

            if seed_ver > cached_ver {
                tracing::info!(
                    "hub: embedded seed v{} > cached seed v{}; upgrading on-disk cache at {}",
                    seed_ver,
                    cached_ver,
                    current.display()
                );
                Self::overwrite_with_seed(&current)?;
            }
            return Ok(());
        }

        fs::create_dir_all(&current).map_err(|e| {
            AppError::internal_error(format!(
                "hub: create {}: {}",
                current.display(),
                e
            ))
        })?;
        Self::dump_dir(&HUB_SEED, &current)?;
        // Mark provenance: this catalog is the embedded seed, not a
        // verified GitHub fetch. A successful refresh removes this
        // marker (the rotated dir never contains it). Surfaced via
        // CatalogProvenance so the UI can show an "offline / built-in"
        // indicator.
        let _ = fs::write(current.join(SEED_MARKER), b"seed\n");
        tracing::info!(
            "hub: installed embedded seed catalog v{} into {}",
            SEED_HUB_VERSION.trim(),
            current.display()
        );
        Ok(())
    }

    /// Wipe the current/ directory and re-dump the embedded seed. Used
    /// for both first-install and the seed-version-upgrade path so the
    /// disk shape (.seed marker, dumped files, mtimes) is identical.
    fn overwrite_with_seed(current: &std::path::Path) -> Result<(), AppError> {
        // Remove every entry under current/ but keep current/ itself —
        // mirrors the dir's semantics (current dir is referenced by
        // `hub_root().join("current")` and may have been opened by
        // other paths).
        if current.exists() {
            for entry in fs::read_dir(current).map_err(|e| {
                AppError::internal_error(format!(
                    "hub: read {}: {}",
                    current.display(),
                    e
                ))
            })? {
                let entry = entry.map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: read {} entry: {}",
                        current.display(),
                        e
                    ))
                })?;
                let path = entry.path();
                let res = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                res.map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: clear {}: {}",
                        path.display(),
                        e
                    ))
                })?;
            }
        } else {
            fs::create_dir_all(current).map_err(|e| {
                AppError::internal_error(format!(
                    "hub: create {}: {}",
                    current.display(),
                    e
                ))
            })?;
        }
        Self::dump_dir(&HUB_SEED, current)?;
        let _ = fs::write(current.join(SEED_MARKER), b"seed\n");
        Ok(())
    }

    /// Where the active catalog came from: the embedded seed (boot
    /// fallback / air-gapped) or a cosign-verified GitHub fetch.
    pub fn provenance(&self) -> CatalogProvenance {
        if self.current_dir().join(SEED_MARKER).exists() {
            CatalogProvenance::Seed
        } else {
            CatalogProvenance::Github
        }
    }

    /// Wall-clock time the active catalog was installed — the mtime of
    /// `current/index.json` (written on seed install + on every fetch
    /// rotate). None if unreadable.
    pub fn last_refreshed(&self) -> Option<DateTime<Utc>> {
        let meta = fs::metadata(self.current_dir().join("index.json")).ok()?;
        let modified = meta.modified().ok()?;
        Some(DateTime::<Utc>::from(modified))
    }

    /// Read the on-disk `index.json`. Errors are surfaced as
    /// `internal_error` — the seed install on boot guarantees the file
    /// exists in a healthy install.
    pub async fn catalog(&self) -> Result<Catalog, AppError> {
        let path = self.current_dir().join("index.json");
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            AppError::internal_error(format!(
                "hub: read index.json at {}: {}",
                path.display(),
                e
            ))
        })?;
        serde_json::from_slice::<Catalog>(&bytes).map_err(|e| {
            AppError::internal_error(format!("hub: parse index.json: {}", e))
        })
    }

    /// Read a per-id manifest YAML from the on-disk catalog. The
    /// category narrows the search — same id may not exist in multiple
    /// categories (validator enforces global uniqueness), but resolving
    /// by `(category, id)` keeps the path lookup deterministic.
    pub async fn manifest(
        &self,
        category: HubCategory,
        id: &str,
    ) -> Result<HubManifest, AppError> {
        if !is_safe_id(id) {
            return Err(AppError::bad_request(
                "HUB_INVALID_ID",
                "hub item id contains characters outside the allowed set [a-z0-9._-]",
            ));
        }
        let folder = category_folder(category);
        let path = self
            .current_dir()
            .join(folder)
            .join(format!("{}.yaml", id));
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                AppError::not_found(&format!("hub manifest {}/{}", folder, id))
            } else {
                AppError::internal_error(format!(
                    "hub: read manifest {}: {}",
                    path.display(),
                    e
                ))
            }
        })?;
        match category {
            HubCategory::Model => {
                let m: HubModel = serde_yaml::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!("hub: parse model {}: {}", id, e))
                })?;
                Ok(HubManifest::model(m))
            }
            HubCategory::Assistant => {
                let a: HubAssistant = serde_yaml::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse assistant {}: {}",
                        id, e
                    ))
                })?;
                Ok(HubManifest::assistant(a))
            }
            HubCategory::McpServer => {
                let s: HubMCPServer = serde_yaml::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse mcp-server {}: {}",
                        id, e
                    ))
                })?;
                Ok(HubManifest::mcp_server(s))
            }
        }
    }

    /// Convenience: load every item in a category. Backs the existing
    /// `/api/hub/{models,assistants,mcp-servers}` endpoints. Reads
    /// O(items-in-category) files (~5 at v0.0.1) — negligible at v1 scale.
    pub async fn list_models(&self) -> Result<Vec<HubModel>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog.items.iter().filter(|i| matches!(i.category, HubCategory::Model)) {
            if let Some(m) = self.manifest(item.category, &item.id).await?.model {
                out.push(*m);
            }
        }
        Ok(out)
    }

    pub async fn list_assistants(&self) -> Result<Vec<HubAssistant>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog.items.iter().filter(|i| matches!(i.category, HubCategory::Assistant)) {
            if let Some(a) = self.manifest(item.category, &item.id).await?.assistant {
                out.push(*a);
            }
        }
        Ok(out)
    }

    pub async fn list_mcp_servers(&self) -> Result<Vec<HubMCPServer>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog.items.iter().filter(|i| matches!(i.category, HubCategory::McpServer)) {
            if let Some(s) = self.manifest(item.category, &item.id).await?.mcp_server {
                out.push(*s);
            }
        }
        Ok(out)
    }

    /// Combined load — backs the old `load_hub_data_with_locale` callers
    /// that read everything in one shot. The `_locale` arg is accepted
    /// for source-compat with the prior shape but ignored: the new
    /// catalog ships English-only at v1; localization is deferred.
    pub async fn load_hub_data_with_locale(
        &self,
        _locale: &str,
    ) -> Result<HubData, AppError> {
        let catalog = self.catalog().await?;
        let models = self.list_models().await?;
        let assistants = self.list_assistants().await?;
        let mcp_servers = self.list_mcp_servers().await?;
        Ok(HubData {
            version: catalog.hub_version,
            models,
            assistants,
            mcp_servers,
        })
    }

    /// The active catalog's `hub_version`.
    pub async fn current_version(&self) -> Result<String, AppError> {
        let catalog = self.catalog().await?;
        Ok(catalog.hub_version)
    }

    /// Reject installing a hub item whose `min_ziee_version` exceeds the
    /// running server. Defense-in-depth behind the UI's hiding of
    /// incompatible items — an API client (or a stale UI) must not be
    /// able to install one. Items absent from the index (orphans /
    /// dev) are treated as installable.
    pub async fn ensure_installable(
        &self,
        category: HubCategory,
        id: &str,
    ) -> Result<(), AppError> {
        let catalog = self.catalog().await?;
        let Some(item) = catalog
            .items
            .iter()
            .find(|it| it.category == category && it.id == id)
        else {
            return Ok(());
        };
        match Self::compat(item) {
            Compat::Ok => Ok(()),
            Compat::TooOld { required } => Err(AppError::unprocessable_entity(
                "HUB_INCOMPATIBLE",
                format!(
                    "hub item '{}' requires ziee >= {} but this server is {}",
                    id,
                    required,
                    server_version()
                ),
            )),
        }
    }

    /// Compatibility check for a single index entry.
    pub fn compat(item: &IndexItem) -> Compat {
        Self::compat_for_server(item, server_version())
    }

    /// Same as `compat` but explicit server version (used in tests so
    /// older or newer fixtures can be exercised against a fixed
    /// `min_ziee_version`).
    pub fn compat_for_server(item: &IndexItem, server_ver: &str) -> Compat {
        let Some(required) = item.min_ziee_version.as_deref() else {
            return Compat::Ok;
        };
        let server = match semver::Version::parse(server_ver) {
            Ok(v) => v,
            Err(_) => return Compat::Ok, // garbled server version → don't block; logged elsewhere
        };
        let req = match semver::Version::parse(required) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "hub: item {:?} has malformed min_ziee_version {:?}: {}",
                    item.id,
                    required,
                    e
                );
                return Compat::Ok;
            }
        };
        if server >= req {
            Compat::Ok
        } else {
            Compat::TooOld {
                required: required.to_string(),
            }
        }
    }

    /// Force-refresh the catalog from GitHub Releases.
    ///
    /// `target`:
    ///   - `None` → fetch the latest release (newest stable, else newest
    ///     prerelease). This is "track latest".
    ///   - `Some("0.0.2-alpha")` → fetch exactly that version (the tag is
    ///     `v` + version). This is the admin-pinned path.
    ///
    /// Returns the previous + new version. Cosign + sha256 failure
    /// aborts; the previous `current/` is left untouched.
    pub async fn refresh(&self, target: Option<String>) -> Result<RefreshOutcome, AppError> {
        // Serialize refreshes process-wide: concurrent refresh/activate
        // calls share the `.staging` / `.previous` dirs and would clobber
        // each other's `remove_dir_all` + `rename`. (activate() calls
        // refresh(), so this guard covers both.)
        let _guard = HUB_REFRESH_LOCK.lock().await;

        let previous_version = self.catalog().await.ok().map(|c| c.hub_version);

        let app_data = self.app_data_dir.clone();
        let outcome = tokio::task::spawn_blocking(move || refresh_blocking(&app_data, target))
            .await
            .map_err(|e| AppError::internal_error(format!("hub: refresh join: {}", e)))??;

        Ok(RefreshOutcome {
            updated: previous_version.as_deref() != Some(outcome.new_version.as_str()),
            previous_version,
            new_version: outcome.new_version,
            cosign_verified: outcome.cosign_verified,
            refreshed_at: Utc::now(),
        })
    }

    /// List the catalog versions published on GitHub Releases. Newest
    /// first. Used by the admin version picker.
    pub async fn list_releases(&self) -> Result<Vec<HubReleaseInfo>, AppError> {
        let releases = tokio::task::spawn_blocking(list_releases_blocking)
            .await
            .map_err(|e| AppError::internal_error(format!("hub: list-releases join: {}", e)))??;
        Ok(releases
            .into_iter()
            .map(|r| HubReleaseInfo {
                version: r.tag_name.trim_start_matches('v').to_string(),
                tag: r.tag_name,
                prerelease: r.prerelease,
                published_at: r.published_at,
            })
            .collect())
    }

    // ----- helpers -----

    /// Copy an `include_dir::Dir` recursively onto disk, overwriting on
    /// hit. Used by both `initialize()` (seed install) and never the
    /// fetch path (that's `tar::Archive` directly).
    fn dump_dir(dir: &Dir<'_>, target: &Path) -> Result<(), AppError> {
        fs::create_dir_all(target).map_err(|e| {
            AppError::internal_error(format!(
                "hub: mkdir {}: {}",
                target.display(),
                e
            ))
        })?;
        for entry in dir.entries() {
            match entry {
                include_dir::DirEntry::File(f) => {
                    let dest = target.join(f.path().strip_prefix(dir.path()).unwrap_or(f.path()));
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent).map_err(|e| {
                            AppError::internal_error(format!(
                                "hub: mkdir {}: {}",
                                parent.display(),
                                e
                            ))
                        })?;
                    }
                    fs::write(&dest, f.contents()).map_err(|e| {
                        AppError::internal_error(format!(
                            "hub: write {}: {}",
                            dest.display(),
                            e
                        ))
                    })?;
                }
                include_dir::DirEntry::Dir(sub) => {
                    let dest = target.join(
                        sub.path()
                            .strip_prefix(dir.path())
                            .unwrap_or(sub.path()),
                    );
                    Self::dump_dir(sub, &dest)?;
                }
            }
        }
        Ok(())
    }
}

// =====================================================================
// Refresh path (blocking — runs on spawn_blocking worker thread)
// =====================================================================

struct BlockingOutcome {
    new_version: String,
    cosign_verified: bool,
}

fn refresh_blocking(app_data: &Path, target: Option<String>) -> Result<BlockingOutcome, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("ziee/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::internal_error(format!("hub: http client: {}", e)))?;

    // Resolve the tag to fetch. A pinned target maps to `v<version>`;
    // None tracks the latest release.
    let tag = match target {
        Some(version) => {
            let v = version.trim_start_matches('v');
            if !is_safe_version(v) {
                return Err(AppError::bad_request(
                    "HUB_INVALID_VERSION",
                    "pinned hub version is not a safe semver-ish string",
                ));
            }
            format!("v{}", v)
        }
        None => resolve_latest_release(&client)?.tag_name,
    };

    let hub_root = hub_root_for(app_data);
    let staging = hub_root.join(".staging");
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    fs::create_dir_all(&staging).map_err(|e| {
        AppError::internal_error(format!(
            "hub: create staging {}: {}",
            staging.display(),
            e
        ))
    })?;

    let assets = [
        "hub.tar.gz",
        "hub.tar.gz.sha256",
        "hub.tar.gz.cosign.bundle",
        "hub.index.json",
        "hub.index.json.sha256",
        "hub.index.json.cosign.bundle",
    ];
    // In a debug build with ZIEE_HUB_ALLOW_UNSIGNED=1 the mock server
    // only publishes the tarball + index + their sha256 (no cosign
    // bundles). Skip downloading bundles we won't verify.
    let unsigned = allow_unsigned();
    for asset in assets {
        if unsigned && asset.ends_with(".cosign.bundle") {
            continue;
        }
        let url = format!(
            "{}/{}/{}/releases/download/{}/{}",
            hub_download_base(),
            HUB_REPO_OWNER,
            HUB_REPO_NAME,
            tag,
            asset
        );
        download_to_file(&client, &url, &staging.join(asset))?;
    }

    let tar_path = staging.join("hub.tar.gz");
    let index_path = staging.join("hub.index.json");

    // sha256 both.
    verify_sha256_sidecar(&tar_path, &staging.join("hub.tar.gz.sha256"))?;
    verify_sha256_sidecar(&index_path, &staging.join("hub.index.json.sha256"))?;

    // cosign keyless both, fail-closed (no signed=false fallback in
    // release). In a debug build with ZIEE_HUB_ALLOW_UNSIGNED=1 (mock
    // release server), skip — the mock can't mint a real Sigstore bundle.
    if unsigned {
        tracing::warn!(
            "hub: ZIEE_HUB_ALLOW_UNSIGNED set (debug) — skipping cosign verify for {}",
            tag
        );
    }
    let identity = cosign_expected_identity(&tag);
    let cosign_verified = if unsigned {
        false
    } else {
        match (
        verify_cosign_bundle(
            &staging.join("hub.tar.gz.cosign.bundle"),
            &tar_path,
            &identity,
            COSIGN_OIDC_ISSUER,
        ),
        verify_cosign_bundle(
            &staging.join("hub.index.json.cosign.bundle"),
            &index_path,
            &identity,
            COSIGN_OIDC_ISSUER,
        ),
    ) {
        (Ok(()), Ok(())) => true,
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(
                "hub.catalog_rejected: cosign verification failed for tag {}: {}",
                tag, e
            );
            return Err(AppError::internal_error(format!(
                "hub: cosign verify failed for {}: {}",
                tag, e
            )));
        }
        }
    };

    // Parse the verified index to confirm the tag matches what release.yml
    // claimed inside the payload. Mismatch isn't a security failure
    // (cosign signed both files together), but it indicates a tagging
    // bug worth surfacing.
    let index_bytes = fs::read(&index_path).map_err(|e| {
        AppError::internal_error(format!("hub: read verified index: {}", e))
    })?;
    let catalog: Catalog = serde_json::from_slice(&index_bytes)
        .map_err(|e| AppError::internal_error(format!("hub: parse verified index: {}", e)))?;
    let expected_version = tag.trim_start_matches('v');
    if catalog.hub_version != expected_version {
        tracing::warn!(
            "hub: tag {} but bundle reports hub_version {}; using tag as authoritative",
            tag, catalog.hub_version
        );
    }

    // Unpack tarball into staging/contents/.
    let contents = staging.join("contents");
    fs::create_dir_all(&contents).map_err(|e| {
        AppError::internal_error(format!(
            "hub: create unpack dir {}: {}",
            contents.display(),
            e
        ))
    })?;
    unpack_safely(&tar_path, &contents)?;

    // Drop the verified index.json into the unpacked contents (the
    // tarball already contains it at the root, but re-writing the
    // verified copy guarantees we never serve content that diverged
    // from the signed file).
    fs::copy(&index_path, contents.join("index.json")).map_err(|e| {
        AppError::internal_error(format!("hub: copy verified index: {}", e))
    })?;

    // Atomically rotate current/.
    let current = hub_root.join("current");
    let backup = hub_root.join(".previous");
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    if current.exists() {
        fs::rename(&current, &backup).map_err(|e| {
            AppError::internal_error(format!("hub: stash current: {}", e))
        })?;
    }
    if let Err(e) = fs::rename(&contents, &current) {
        // Roll back.
        if backup.exists() {
            let _ = fs::rename(&backup, &current);
        }
        return Err(AppError::internal_error(format!(
            "hub: promote staging → current: {}",
            e
        )));
    }
    // Clean up.
    let _ = fs::remove_dir_all(&backup);
    let _ = fs::remove_dir_all(&staging);

    tracing::info!(
        "hub: refreshed catalog to {} (cosign_verified={})",
        tag, cosign_verified
    );
    Ok(BlockingOutcome {
        new_version: expected_version.to_string(),
        cosign_verified,
    })
}

// =====================================================================
// Verify helpers (ported from code_sandbox/runtime_fetch.rs)
// =====================================================================

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    published_at: Option<String>,
}

/// Fetch the repo's releases (newest first per the GitHub API), drafts
/// filtered out. Shared by `resolve_latest_release` + `list_releases`.
fn fetch_releases(client: &reqwest::blocking::Client) -> Result<Vec<GhRelease>, AppError> {
    let url = format!(
        "{}/repos/{}/{}/releases?per_page=50",
        hub_api_base(),
        HUB_REPO_OWNER,
        HUB_REPO_NAME
    );
    let releases: Vec<GhRelease> = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| AppError::internal_error(format!("hub: list releases: {}", e)))?
        .error_for_status()
        .map_err(|e| AppError::internal_error(format!("hub: list releases: {}", e)))?
        .json()
        .map_err(|e| AppError::internal_error(format!("hub: parse releases: {}", e)))?;
    Ok(releases.into_iter().filter(|r| !r.draft).collect())
}

fn list_releases_blocking() -> Result<Vec<GhRelease>, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("ziee/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::internal_error(format!("hub: http client: {}", e)))?;
    fetch_releases(&client)
}

fn resolve_latest_release(client: &reqwest::blocking::Client) -> Result<GhRelease, AppError> {
    // `/releases/latest` skips prereleases by definition, but we still
    // need to surface them when stable hasn't shipped yet (e.g. during
    // the v0.0.x-alpha window). Strategy: prefer the most recent
    // non-prerelease tag; fall back to the newest prerelease if no
    // stable exists.
    let releases = fetch_releases(client)?;
    if let Some(stable) = releases.iter().find(|r| !r.prerelease) {
        return Ok(stable.clone());
    }
    releases
        .into_iter()
        .next()
        .ok_or_else(|| AppError::internal_error("hub: no releases found on GitHub"))
}

fn download_to_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
) -> Result<u64, AppError> {
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        match client.get(url).send() {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_err = format!("HTTP {}", status);
                    if status.is_server_error() && attempt < 3 {
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    return Err(AppError::internal_error(format!(
                        "hub: download {} failed: {}",
                        url, last_err
                    )));
                }
                if let Some(len) = resp.content_length()
                    && len > MAX_HUB_ARTIFACT_BYTES
                {
                    return Err(AppError::internal_error(format!(
                        "hub: {} declares {} bytes (cap {})",
                        url, len, MAX_HUB_ARTIFACT_BYTES
                    )));
                }
                let mut file = fs::File::create(dest).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: create {}: {}",
                        dest.display(),
                        e
                    ))
                })?;
                let mut resp = resp;
                match resp.copy_to(&mut file) {
                    Ok(n) => return Ok(n),
                    Err(e) => {
                        last_err = format!("stream-to-file: {}", e);
                        let _ = fs::remove_file(dest);
                        if attempt < 3 {
                            std::thread::sleep(Duration::from_secs(2));
                            continue;
                        }
                        return Err(AppError::internal_error(format!(
                            "hub: download {}: {}",
                            url, last_err
                        )));
                    }
                }
            }
            Err(e) => {
                last_err = format!("send: {}", e);
                if attempt < 3 {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                return Err(AppError::internal_error(format!(
                    "hub: download {}: {}",
                    url, last_err
                )));
            }
        }
    }
    Err(AppError::internal_error(format!(
        "hub: download {}: {}",
        url, last_err
    )))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = fs::File::open(path)?;
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

fn verify_sha256_sidecar(blob: &Path, sidecar: &Path) -> Result<(), AppError> {
    let sidecar_text = fs::read_to_string(sidecar).map_err(|e| {
        AppError::internal_error(format!(
            "hub: read sha256 sidecar {}: {}",
            sidecar.display(),
            e
        ))
    })?;
    // sha256sum sidecar shape: "<hex>  <filename>"
    let expected_hex = sidecar_text
        .split_whitespace()
        .next()
        .ok_or_else(|| AppError::internal_error("hub: empty sha256 sidecar"))?
        .to_lowercase();
    // Validate the format up front (matches runtime_fetch.rs). A
    // malformed sidecar would never match the 64-char hex digest below
    // anyway, but failing fast with a clear message beats a confusing
    // mismatch error.
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::internal_error(format!(
            "hub: malformed sha256 in sidecar {}",
            sidecar.display()
        )));
    }
    let actual_hex = sha256_file(blob).map_err(|e| {
        AppError::internal_error(format!(
            "hub: hash blob {}: {}",
            blob.display(),
            e
        ))
    })?;
    if actual_hex != expected_hex {
        return Err(AppError::internal_error(format!(
            "hub: sha256 mismatch for {}: expected {} got {}",
            blob.display(),
            expected_hex,
            actual_hex
        )));
    }
    Ok(())
}

fn verify_cosign_bundle(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    use sigstore::bundle::Bundle;
    use sigstore::bundle::verify::blocking::Verifier;
    use sigstore::bundle::verify::policy::Identity;

    let bundle_json =
        fs::read_to_string(bundle_path).map_err(|e| format!("read bundle: {}", e))?;
    let bundle: Bundle =
        serde_json::from_str(&bundle_json).map_err(|e| format!("parse bundle: {}", e))?;
    let blob = fs::File::open(blob_path).map_err(|e| format!("open blob: {}", e))?;
    let verifier = Verifier::production().map_err(|e| format!("trust root init: {}", e))?;
    let policy = Identity::new(identity, issuer);
    verifier
        .verify(blob, bundle, &policy, false)
        .map_err(|e| format!("signature verification: {}", e))?;
    Ok(())
}

/// Tarball unpack with traversal protection. Refuses entries whose
/// normalized path starts with `..` or contains absolute components.
fn unpack_safely(tar_gz: &Path, dest: &Path) -> Result<(), AppError> {
    let f = fs::File::open(tar_gz).map_err(|e| {
        AppError::internal_error(format!("hub: open {}: {}", tar_gz.display(), e))
    })?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    // Decompression-bomb guards: the 32 MiB cap on `download_to_file`
    // only bounds the COMPRESSED tarball; gzip can expand by orders of
    // magnitude. Bound the cumulative uncompressed size + entry count
    // so a malicious/buggy release can't fill the disk.
    const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
    const MAX_ENTRIES: usize = 100_000;
    let mut total_unpacked: u64 = 0;
    let mut entry_count: usize = 0;

    for entry in archive.entries().map_err(|e| {
        AppError::internal_error(format!("hub: read archive: {}", e))
    })? {
        let mut entry = entry.map_err(|e| {
            AppError::internal_error(format!("hub: read entry: {}", e))
        })?;

        entry_count += 1;
        if entry_count > MAX_ENTRIES {
            return Err(AppError::internal_error(
                "hub: archive exceeds entry-count cap".to_string(),
            ));
        }
        total_unpacked = total_unpacked.saturating_add(entry.header().size().unwrap_or(0));
        if total_unpacked > MAX_UNPACKED_BYTES {
            return Err(AppError::internal_error(
                "hub: archive exceeds uncompressed-size cap".to_string(),
            ));
        }

        // Manifests + schemas + index are all regular files. Reject
        // symlinks/hardlinks outright — a `link -> /etc` entry followed
        // by writes through it is the classic tar symlink escape, and
        // the catalog never legitimately contains links.
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(AppError::internal_error(format!(
                "hub: refusing non-regular archive entry ({:?})",
                kind
            )));
        }

        let path = entry
            .path()
            .map_err(|e| AppError::internal_error(format!("hub: entry path: {}", e)))?
            .into_owned();
        // Reject absolute paths and any `..` traversal component.
        if path.is_absolute() {
            return Err(AppError::internal_error(format!(
                "hub: refusing absolute path inside archive: {}",
                path.display()
            )));
        }
        // Reject `..` AND Windows-style `C:\...` prefix / root
        // components — `Path::is_absolute()` on Linux returns
        // `false` for `C:\evil`, so a tarball produced on Windows
        // could otherwise sneak a root-anchored path through.
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(AppError::internal_error(format!(
                        "hub: refusing parent-dir component in archive: {}",
                        path.display()
                    )));
                }
                std::path::Component::RootDir => {
                    return Err(AppError::internal_error(format!(
                        "hub: refusing root-dir component in archive: {}",
                        path.display()
                    )));
                }
                std::path::Component::Prefix(_) => {
                    return Err(AppError::internal_error(format!(
                        "hub: refusing windows-prefix component in archive: {}",
                        path.display()
                    )));
                }
                std::path::Component::CurDir => {
                    return Err(AppError::internal_error(format!(
                        "hub: refusing cur-dir component in archive: {}",
                        path.display()
                    )));
                }
                _ => {}
            }
        }
        entry.unpack_in(dest).map_err(|e| {
            AppError::internal_error(format!(
                "hub: unpack {}: {}",
                path.display(),
                e
            ))
        })?;
    }
    Ok(())
}

// =====================================================================
// Misc helpers
// =====================================================================

fn category_folder(category: HubCategory) -> &'static str {
    match category {
        HubCategory::Model => "models",
        HubCategory::Assistant => "assistants",
        HubCategory::McpServer => "mcp-servers",
    }
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| {
            b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_'
        })
        && !id.starts_with('.')
}

/// Guard a pinned version string before it's interpolated into a
/// GitHub Releases download URL (`releases/download/v<version>/...`).
/// Rejects anything that could break out of the path or smuggle URL
/// syntax — must look like `0.0.2` / `1.2.3-alpha.1`.
fn is_safe_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 32
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !v.starts_with('.')
        && !v.starts_with('-')
        && v.chars().next().is_some_and(|c| c.is_ascii_digit())
}

// =====================================================================
// Unit tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ix(id: &str, min: Option<&str>) -> IndexItem {
        IndexItem {
            id: id.to_string(),
            category: HubCategory::Model,
            name: id.to_string(),
            summary: String::new(),
            tags: vec![],
            verified: false,
            added_at: None,
            min_ziee_version: min.map(String::from),
            manifest_path: format!("models/{id}.yaml"),
        }
    }

    #[test]
    fn compat_ok_when_no_min_version() {
        assert_eq!(
            HubManager::compat_for_server(&ix("foo", None), "0.1.0"),
            Compat::Ok
        );
    }

    #[test]
    fn compat_ok_when_server_equals_required() {
        assert_eq!(
            HubManager::compat_for_server(&ix("foo", Some("0.1.0")), "0.1.0"),
            Compat::Ok
        );
    }

    #[test]
    fn compat_ok_when_server_newer_than_required() {
        assert_eq!(
            HubManager::compat_for_server(&ix("foo", Some("0.1.0")), "0.5.0"),
            Compat::Ok
        );
    }

    #[test]
    fn compat_too_old_when_server_older_than_required() {
        let item = ix("foo", Some("0.5.0"));
        let got = HubManager::compat_for_server(&item, "0.1.0");
        assert_eq!(
            got,
            Compat::TooOld {
                required: "0.5.0".to_string()
            }
        );
        assert!(!got.is_ok());
    }

    #[test]
    fn compat_ok_when_min_version_is_garbage() {
        // Don't block on contributor mistakes — log + treat as compatible.
        assert_eq!(
            HubManager::compat_for_server(&ix("foo", Some("not-a-version")), "0.1.0"),
            Compat::Ok
        );
    }

    #[test]
    fn is_safe_id_rejects_path_traversal() {
        assert!(is_safe_id("llama-3-1-8b-instruct"));
        assert!(is_safe_id("foo.bar"));
        assert!(is_safe_id("foo_bar"));
        assert!(!is_safe_id("../etc/passwd"));
        assert!(!is_safe_id("foo/bar"));
        assert!(!is_safe_id(".hidden"));
        assert!(!is_safe_id(""));
        assert!(!is_safe_id(&"a".repeat(65)));
    }

    #[test]
    fn is_safe_version_accepts_semver_rejects_injection() {
        assert!(is_safe_version("0.0.2"));
        assert!(is_safe_version("1.2.3-alpha.1"));
        assert!(is_safe_version("0.0.1-alpha"));
        // leading-v stripped by callers, but bare v must fail the digit gate
        assert!(!is_safe_version("v0.0.2"));
        assert!(!is_safe_version("../../etc"));
        assert!(!is_safe_version("0.0.2/../../x"));
        assert!(!is_safe_version("0.0.2?foo=bar"));
        assert!(!is_safe_version(""));
        assert!(!is_safe_version("-rc1"));
        assert!(!is_safe_version(".hidden"));
        assert!(!is_safe_version(&"9".repeat(33)));
    }

    #[test]
    fn category_folder_is_stable() {
        assert_eq!(category_folder(HubCategory::Model), "models");
        assert_eq!(category_folder(HubCategory::Assistant), "assistants");
        assert_eq!(category_folder(HubCategory::McpServer), "mcp-servers");
    }

    #[test]
    fn server_version_matches_pkg_version() {
        // sanity: env! works and returns a parseable semver
        assert!(semver::Version::parse(server_version()).is_ok());
    }

    #[test]
    fn cosign_expected_identity_includes_tag() {
        let s = cosign_expected_identity("v0.0.1-alpha");
        assert!(s.contains("ziee-ai/hub"));
        assert!(s.contains("release.yml"));
        assert!(s.ends_with("@refs/tags/v0.0.1-alpha"));
    }

    #[test]
    fn seed_manifest_yaml_round_trips_into_structs() {
        // Pull a real manifest out of the embedded seed and parse it
        // into the typed struct — guards the YAML field mapping (the
        // manifests are authored in the hub repo, consumed here).
        let model_yaml = HUB_SEED
            .get_file("models/llama-3-1-8b-instruct.yaml")
            .expect("seed has llama model");
        let model: HubModel =
            serde_yaml::from_slice(model_yaml.contents()).expect("parse model yaml");
        assert_eq!(model.id, "llama-3-1-8b-instruct");
        assert!(matches!(model.file_format, super::super::models::FileFormat::SafeTensors));

        let asst_yaml = HUB_SEED
            .get_file("assistants/code-reviewer.yaml")
            .expect("seed has code-reviewer");
        let asst: HubAssistant =
            serde_yaml::from_slice(asst_yaml.contents()).expect("parse assistant yaml");
        assert_eq!(asst.id, "code-reviewer");

        let mcp_yaml = HUB_SEED
            .get_file("mcp-servers/github-mcp.yaml")
            .expect("seed has github-mcp");
        let mcp: HubMCPServer =
            serde_yaml::from_slice(mcp_yaml.contents()).expect("parse mcp yaml");
        assert_eq!(mcp.id, "github-mcp");
        assert_eq!(mcp.transport_type.as_deref(), Some("http"));
    }

    #[test]
    fn sha256_file_and_sidecar_verify() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("hub-sha-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let blob = dir.join("blob.bin");
        let mut f = fs::File::create(&blob).unwrap();
        f.write_all(b"ziee hub test payload").unwrap();
        drop(f);

        // Known sha256 of the payload above.
        let hex = sha256_file(&blob).unwrap();
        assert_eq!(hex.len(), 64);

        // A matching sidecar verifies; a tampered one fails.
        let sidecar = dir.join("blob.bin.sha256");
        fs::write(&sidecar, format!("{}  blob.bin\n", hex)).unwrap();
        assert!(verify_sha256_sidecar(&blob, &sidecar).is_ok());

        fs::write(&sidecar, format!("{}  blob.bin\n", "0".repeat(64))).unwrap();
        assert!(verify_sha256_sidecar(&blob, &sidecar).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_directory_carries_index_and_categories() {
        // Compile-time check that the seed was staged correctly. If
        // resources/hub-seed/ is missing or empty the build would have
        // failed at include_dir!, but this test catches a partial seed
        // (e.g. missing categories) at unit test time.
        let names: Vec<_> = HUB_SEED
            .entries()
            .iter()
            .filter_map(|e| e.path().file_name().and_then(|s| s.to_str()))
            .collect();
        assert!(names.contains(&"index.json"), "seed missing index.json: {names:?}");
        assert!(names.contains(&"models"));
        assert!(names.contains(&"assistants"));
        assert!(names.contains(&"mcp-servers"));
    }

    // ─────────────────────────────────────────────────────────────────
    // initialize() — seed-upgrade-on-boot matrix
    // ─────────────────────────────────────────────────────────────────

    /// Build a temp app-data-dir whose `hub/current/` already has a
    /// hand-rolled `index.json` at an arbitrary `hub_version`, with
    /// the `.seed` marker present iff `seed_provenance` is true.
    /// Returns the data dir (auto-cleaned by the caller).
    fn fixture_with_existing_catalog(version: &str, seed_provenance: bool) -> PathBuf {
        let unique = format!(
            "hub-init-{}-{}",
            std::process::id(),
            // Use the version string + provenance as a stable suffix —
            // tests don't run in parallel here (each writes its own
            // tempdir) but avoid Date::now() per CLAUDE.md.
            version.replace('.', "_"),
        );
        let data_dir = std::env::temp_dir().join(format!("{unique}-{seed_provenance}"));
        let current = data_dir.join("hub").join("current");
        fs::create_dir_all(&current).unwrap();
        // Minimal valid Catalog shape (matches struct field set).
        let body = serde_json::json!({
            "hub_version": version,
            "generated_at": "1970-01-01T00:00:00Z",
            "schema_version": 1,
            "items": [],
        });
        fs::write(
            current.join("index.json"),
            serde_json::to_vec(&body).unwrap(),
        )
        .unwrap();
        if seed_provenance {
            fs::write(current.join(SEED_MARKER), b"seed\n").unwrap();
        }
        data_dir
    }

    fn cached_version(data_dir: &std::path::Path) -> String {
        let path = data_dir.join("hub").join("current").join("index.json");
        let bytes = fs::read(&path).unwrap();
        let cat: Catalog = serde_json::from_slice(&bytes).unwrap();
        cat.hub_version
    }

    #[test]
    fn initialize_upgrades_stale_seed_cache_to_embedded_seed_version() {
        // Cache is from a previous boot that seeded v0.0.0-alpha; the
        // current binary embeds something newer (whatever
        // SEED_HUB_VERSION resolves to). The upgrade path MUST replace
        // the on-disk catalog with the embedded seed.
        //
        // 0.0.0-alpha is always strictly less than any real published
        // seed (releases start at 0.0.1-alpha), so this assertion is
        // version-independent.
        let dir = fixture_with_existing_catalog("0.0.0-alpha", true);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(
            cached_version(&dir),
            SEED_HUB_VERSION.trim(),
            "stale seed cache should be upgraded to the embedded seed version"
        );
        // Marker preserved — still a seed install.
        assert!(dir.join("hub").join("current").join(SEED_MARKER).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_leaves_same_version_seed_cache_alone() {
        // Cache version == seed version → no-op (no churn on every
        // boot). Use SEED_HUB_VERSION verbatim so the test stays
        // correct across version bumps.
        let v = SEED_HUB_VERSION.trim();
        let dir = fixture_with_existing_catalog(v, true);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(cached_version(&dir), v);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_leaves_github_fetched_cache_alone_even_when_older() {
        // No `.seed` marker → provenance is Github. Operator-managed
        // catalog (possibly intentionally older for compat) must NEVER
        // be auto-replaced by the embedded seed.
        let dir = fixture_with_existing_catalog("0.0.0-alpha", /* seed */ false);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(
            cached_version(&dir),
            "0.0.0-alpha",
            "GitHub-fetched cache must not be silently rewritten by the seed"
        );
        // Still no marker — provenance preserved.
        assert!(!dir.join("hub").join("current").join(SEED_MARKER).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_installs_seed_on_fresh_data_dir() {
        let dir = std::env::temp_dir().join(format!("hub-init-fresh-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(cached_version(&dir), SEED_HUB_VERSION.trim());
        assert!(dir.join("hub").join("current").join(SEED_MARKER).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_index_version_matches_const() {
        // SEED_HUB_VERSION is hand-maintained; the embedded index.json
        // is generated from the hub repo. If they drift, /version +
        // provenance logic would report the wrong version. Fail the
        // build on mismatch.
        let index = HUB_SEED
            .get_file("index.json")
            .expect("seed has index.json");
        let catalog: Catalog =
            serde_json::from_slice(index.contents()).expect("parse seed index.json");
        assert_eq!(
            catalog.hub_version, SEED_HUB_VERSION,
            "resources/hub-seed/index.json hub_version ({}) != SEED_HUB_VERSION const ({})",
            catalog.hub_version, SEED_HUB_VERSION
        );
    }
}
