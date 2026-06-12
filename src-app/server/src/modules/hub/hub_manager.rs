//! HubManager — Pages-backed catalog of models, assistants, MCP servers.
//!
//! Source of truth: the `gh-pages` branch of `ziee-ai/hub`, served as a
//! static site at `https://ziee-ai.github.io/hub/`. Layout:
//!
//! ```
//!   /index.json                                  # the Catalog
//!   /schemas/v2/*.schema.json                    # versioned schemas
//!   /<type>/<id>/<version>.json                  # full manifest
//! ```
//!
//! `refresh()` only fetches `index.json` (the lightweight envelope
//! list — every list/card view reads from this); full manifests are
//! pulled lazily by `manifest()` only when an entry is opened or
//! installed, and cached on disk version-addressed so multiple
//! versions can coexist.
//!
//! Boot fallback: an embedded seed under `binaries/hub-seed/` (mirror
//! of `resources/hub-seed/` — copied verbatim by
//! `build_helper/hub_seed.rs` at compile time) is installed on first
//! run so the hub UI works fully offline. The seed is the only
//! pre-network state; `current/` is then either re-seeded on upgrade
//! or replaced by a `refresh()` from Pages.
//!
//! Trust model: HTTPS to GitHub Pages is the boundary. No cosign / no
//! sha256 sidecars. JSON Schema validation + per-fetch size cap are
//! the only payload safety checks (see `MAX_HUB_ARTIFACT_BYTES`).

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

/// Default base URL the Pages branch is served from. Overridable in
/// debug builds (only) via `ZIEE_HUB_PAGES_BASE`.
pub const DEFAULT_PAGES_BASE: &str = "https://ziee-ai.github.io/hub";

/// Hard cap on any single fetched JSON. The catalog ships ~10 KB at
/// v2.0.0 (5 entries) and grows linearly with item count; 32 MiB
/// leaves headroom for thousands of entries while preventing a
/// redirect from filling memory via `.bytes().await`.
const MAX_HUB_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;

/// HTTP timeout for any single Pages fetch.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

// =====================================================================
// Dev/test overrides — physically compiled OUT of release builds via
// `cfg!(debug_assertions)`, same pattern code_sandbox's dev-mirror
// uses. Let the integration suite point the fetcher at a local mock
// Pages server (no network) without any risk in shipped binaries.
// =====================================================================

/// Pages site base. Override in debug via `ZIEE_HUB_PAGES_BASE`
/// (e.g. `http://127.0.0.1:PORT`).
fn hub_pages_base() -> String {
    if cfg!(debug_assertions)
        && let Ok(v) = std::env::var("ZIEE_HUB_PAGES_BASE")
        && !v.is_empty()
    {
        return v;
    }
    DEFAULT_PAGES_BASE.to_string()
}

/// On-disk root of the hub catalog (`current/`, `.staging/`). Debug
/// builds honor `ZIEE_HUB_DATA_DIR_OVERRIDE` so the integration suite
/// can give each test an isolated catalog dir — the catalog is
/// per-deployment global mutable state, so without isolation a
/// mutating test contaminates every other test sharing the same
/// app_data dir. Always `app_data/hub` in release.
fn hub_root_for(app_data: &Path) -> PathBuf {
    if cfg!(debug_assertions)
        && let Ok(d) = std::env::var("ZIEE_HUB_DATA_DIR_OVERRIDE")
        && !d.is_empty()
    {
        return PathBuf::from(d);
    }
    app_data.join("hub")
}

/// Server semver. The `compat()` helper compares this against each
/// IndexItem's `min_ziee_version` to partition the catalog into
/// installable vs incompatible.
pub fn server_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// =====================================================================
// Embedded seed (compile-time copy of `resources/hub-seed/`)
// =====================================================================

static HUB_SEED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/binaries/hub-seed");

/// Seed catalog version. Written by `build_helper/hub_seed.rs` from
/// the tracked seed's `index.json` `hub_version` field, so this const
/// stays in lockstep with whatever `binaries/hub-seed/` actually
/// contains.
pub const SEED_HUB_VERSION: &str =
    include_str!(concat!(env!("OUT_DIR"), "/hub_seed_version.txt"));

/// Marker file in `current/` indicating the active catalog is the
/// embedded seed (never fetched from Pages). A successful refresh
/// removes this marker.
const SEED_MARKER: &str = ".seed";

/// Process-wide lock serializing catalog refresh so concurrent
/// callers don't clobber the index swap.
static HUB_REFRESH_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

/// Origin of the active on-disk catalog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CatalogProvenance {
    /// Embedded seed (boot fallback / air-gapped). Trusted (compiled
    /// into the binary) and installable, but not a live fetch.
    Seed,
    /// Fetched from the Pages branch at `hub_pages_base()`. HTTPS-only
    /// trust — no cosign / sha256.
    Pages,
}

// =====================================================================
// Catalog types (returned from `catalog()` / consumed by handlers)
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Catalog {
    pub schema_version: u32,
    /// Build marker stamped by the publisher. Under v2 this is NOT the
    /// per-entry update signal — per-entry `IndexItem.version` is the
    /// truth. Kept for diagnostics + the seed-version test guard.
    pub hub_version: String,
    #[serde(default)]
    pub generated_at: Option<String>,
    pub items: Vec<IndexItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexItem {
    /// v2 envelope — reverse-DNS canonical name, unique across sources.
    /// ziee-native entries use `io.github.<contributor>/<slug>`; ingested
    /// MCP entries keep their official `name`. Matches the per-entry
    /// manifest's top-level `name` field; used as the lookup key for
    /// `manifest()` / `ensure_installable()` / install requests
    /// (the `hub_id` field on `/hub/*/create` is this value).
    pub name: String,
    pub category: HubCategory,
    /// Human display label (replaces v1 `IndexItem.name`). Optional —
    /// cards fall back to the slug portion of `name` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    pub added_at: Option<String>,
    pub min_ziee_version: Option<String>,
    /// Pages-relative path to the full manifest, e.g.
    /// `"models/io.github.ziee-ai/llama-3-8b-instruct/1.0.0.json"`.
    /// Used as both the HTTP fetch suffix and the on-disk cache path.
    /// Validated by `is_safe_manifest_path` before any file or URL use.
    pub manifest_path: String,
    /// v2 envelope — per-entry semver. Replaces the role of the
    /// monolithic `Catalog.hub_version` as the update signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// v2 envelope — namespaced extras (e.g.
    /// `io.modelcontextprotocol.registry/*` on ingested entries).
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

/// Full manifest for one hub item, returned by `GET /api/hub/manifest/:id`.
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
    Ok,
    TooOld { required: String },
}

impl Compat {
    pub fn is_ok(&self) -> bool {
        matches!(self, Compat::Ok)
    }
}

/// Returned from `refresh()` so handlers can surface "no change" vs
/// "advanced to v0.X.Y" in toasts. v1 carried a `cosign_verified`
/// field; v2 dropped it (HTTPS-only trust).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefreshOutcome {
    pub previous_version: Option<String>,
    pub new_version: String,
    pub updated: bool,
    pub refreshed_at: DateTime<Utc>,
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

    /// On boot: install the embedded seed catalog into
    /// `<app_data>/hub/current/` if it doesn't already exist; also
    /// upgrade a stale seed-provenance cache when the binary embeds a
    /// newer `SEED_HUB_VERSION`.
    ///
    /// A Pages-fetched cache (provenance == Pages) is NEVER auto-
    /// replaced — it's been pulled by an operator action and may
    /// legitimately be newer than the embedded seed.
    pub fn initialize(&self) -> Result<(), AppError> {
        let current = self.current_dir();
        let index_path = current.join("index.json");

        if !index_path.exists() {
            // Fresh install — fall through to install the seed.
        } else {
            let is_seed = current.join(SEED_MARKER).exists();
            if !is_seed {
                // Pages-fetched cache — operator's source of truth.
                return Ok(());
            }
            let cached_version = match fs::read(&index_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Catalog>(&bytes).ok())
            {
                Some(cat) => cat.hub_version,
                None => {
                    tracing::warn!(
                        "hub: seed cache at {} is unreadable; reseeding",
                        index_path.display()
                    );
                    Self::overwrite_with_seed(&current)?;
                    return Ok(());
                }
            };

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
        let _ = fs::write(current.join(SEED_MARKER), b"seed\n");
        tracing::info!(
            "hub: installed embedded seed catalog v{} into {}",
            SEED_HUB_VERSION.trim(),
            current.display()
        );
        Ok(())
    }

    /// Wipe the current/ directory and re-dump the embedded seed.
    fn overwrite_with_seed(current: &std::path::Path) -> Result<(), AppError> {
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

    /// Where the active catalog came from.
    pub fn provenance(&self) -> CatalogProvenance {
        if self.current_dir().join(SEED_MARKER).exists() {
            CatalogProvenance::Seed
        } else {
            CatalogProvenance::Pages
        }
    }

    /// Wall-clock time the active catalog was installed.
    pub fn last_refreshed(&self) -> Option<DateTime<Utc>> {
        let meta = fs::metadata(self.current_dir().join("index.json")).ok()?;
        let modified = meta.modified().ok()?;
        Some(DateTime::<Utc>::from(modified))
    }

    /// Read the on-disk `index.json`.
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

    /// Read the per-entry manifest. The path is resolved from the
    /// catalog's `manifest_path` for the matching `(category, name)`
    /// IndexItem, so per-version layout
    /// (`<folder>/<namespace>/<leaf>/<version>.json`) is the publisher's
    /// choice — the consumer just follows the link.
    ///
    /// `name` is the reverse-DNS canonical name (e.g.
    /// `io.github.modelcontextprotocol/filesystem`); validated via
    /// `is_safe_name` before lookup.
    ///
    /// Lazy: if the cache file is missing AND the catalog reports a
    /// Pages provenance for the running cache, the per-entry manifest
    /// is fetched from `<base>/<manifest_path>` and written into the
    /// cache version-addressed before returning.
    pub async fn manifest(
        &self,
        category: HubCategory,
        name: &str,
    ) -> Result<HubManifest, AppError> {
        if !is_safe_name(name) {
            return Err(AppError::bad_request(
                "HUB_INVALID_ID",
                "hub item name contains characters outside the allowed reverse-DNS set",
            ));
        }

        // Resolve the IndexItem so we know which `manifest_path` to
        // read. Per-entry version + the on-disk filename are the
        // publisher's call; we just look it up.
        let catalog = self.catalog().await?;
        let item = catalog
            .items
            .iter()
            .find(|it| it.category == category && it.name == name)
            .ok_or_else(|| {
                AppError::not_found(&format!(
                    "hub manifest {}/{}",
                    category_folder(category),
                    name
                ))
            })?;

        let rel = &item.manifest_path;
        if !is_safe_manifest_path(rel) {
            return Err(AppError::internal_error(format!(
                "hub: index entry {}/{} has unsafe manifest_path {:?}",
                category_folder(category),
                name,
                rel
            )));
        }

        let cache_path = self.current_dir().join(rel);
        let bytes = match tokio::fs::read(&cache_path).await {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Lazy fetch — pull just this manifest from Pages and
                // cache it version-addressed. Seed-provenance caches
                // SHOULD already carry every entry (the seed ships all
                // of them), so a miss here generally means a
                // Pages-fetched cache that hasn't pulled this entry
                // yet. Fail back to seed lookup if the cache is the
                // seed, in case the seed has the file at a different
                // disk layout than the publisher claims.
                self.fetch_and_cache_manifest(rel).await?
            }
            Err(e) => {
                return Err(AppError::internal_error(format!(
                    "hub: read manifest {}: {}",
                    cache_path.display(),
                    e
                )));
            }
        };

        match category {
            HubCategory::Model => {
                let m: HubModel = serde_json::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!("hub: parse model {}: {}", name, e))
                })?;
                Ok(HubManifest::model(m))
            }
            HubCategory::Assistant => {
                let a: HubAssistant = serde_json::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse assistant {}: {}",
                        name, e
                    ))
                })?;
                Ok(HubManifest::assistant(a))
            }
            HubCategory::McpServer => {
                let s: HubMCPServer = serde_json::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse mcp-server {}: {}",
                        name, e
                    ))
                })?;
                Ok(HubManifest::mcp_server(s))
            }
        }
    }

    /// Convenience: load every item in a category.
    pub async fn list_models(&self) -> Result<Vec<HubModel>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog
            .items
            .iter()
            .filter(|i| matches!(i.category, HubCategory::Model))
        {
            if let Some(m) = self.manifest(item.category, &item.name).await?.model {
                out.push(*m);
            }
        }
        Ok(out)
    }

    pub async fn list_assistants(&self) -> Result<Vec<HubAssistant>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog
            .items
            .iter()
            .filter(|i| matches!(i.category, HubCategory::Assistant))
        {
            if let Some(a) = self.manifest(item.category, &item.name).await?.assistant {
                out.push(*a);
            }
        }
        Ok(out)
    }

    pub async fn list_mcp_servers(&self) -> Result<Vec<HubMCPServer>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog
            .items
            .iter()
            .filter(|i| matches!(i.category, HubCategory::McpServer))
        {
            if let Some(s) = self.manifest(item.category, &item.name).await?.mcp_server {
                out.push(*s);
            }
        }
        Ok(out)
    }

    /// Combined load — backs the old `load_hub_data_with_locale`
    /// callers that read everything in one shot. The `_locale` arg
    /// is accepted for source-compat but ignored.
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

    /// The active catalog's build-marker `hub_version`. Under v2 this
    /// is rarely the right value to stamp on installs (use the
    /// per-entry `IndexItem.version` for that — handlers do this
    /// directly via `catalog().items.find(...).version`).
    pub async fn current_version(&self) -> Result<String, AppError> {
        let catalog = self.catalog().await?;
        Ok(catalog.hub_version)
    }

    /// Reject installing a hub item whose `min_ziee_version` exceeds
    /// the running server. `name` is the reverse-DNS canonical name
    /// (matches `IndexItem.name`).
    pub async fn ensure_installable(
        &self,
        category: HubCategory,
        name: &str,
    ) -> Result<(), AppError> {
        let catalog = self.catalog().await?;
        let Some(item) = catalog
            .items
            .iter()
            .find(|it| it.category == category && it.name == name)
        else {
            return Ok(());
        };
        match Self::compat(item) {
            Compat::Ok => Ok(()),
            Compat::TooOld { required } => Err(AppError::unprocessable_entity(
                "HUB_INCOMPATIBLE",
                format!(
                    "hub item '{}' requires ziee >= {} but this server is {}",
                    name,
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

    pub fn compat_for_server(item: &IndexItem, server_ver: &str) -> Compat {
        let Some(required) = item.min_ziee_version.as_deref() else {
            return Compat::Ok;
        };
        let server = match semver::Version::parse(server_ver) {
            Ok(v) => v,
            Err(_) => return Compat::Ok,
        };
        let req = match semver::Version::parse(required) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "hub: item {:?} has malformed min_ziee_version {:?}: {}",
                    item.name,
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

    /// Refresh the catalog index from Pages.
    ///
    /// v2 is index-only: per-entry manifests are fetched lazily on
    /// demand (see `manifest()`), so refresh just GETs `index.json`,
    /// validates it parses, and atomically replaces
    /// `current/index.json`. The seed marker is cleared on success,
    /// flipping the provenance to `Pages`.
    pub async fn refresh(&self) -> Result<RefreshOutcome, AppError> {
        let _guard = HUB_REFRESH_LOCK.lock().await;

        let previous_version = self.catalog().await.ok().map(|c| c.hub_version);

        let base = hub_pages_base();
        let url = format!("{}/index.json", base.trim_end_matches('/'));
        let bytes = tokio::task::spawn_blocking(move || download_json(&url))
            .await
            .map_err(|e| AppError::internal_error(format!("hub: refresh join: {}", e)))??;

        let catalog: Catalog = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::internal_error(format!("hub: parse fetched index.json: {}", e))
        })?;

        // Atomic-ish swap: write tmp, fs::rename over current/index.json,
        // then drop the seed marker (if any) since we're now Pages-backed.
        let current = self.current_dir();
        fs::create_dir_all(&current).map_err(|e| {
            AppError::internal_error(format!(
                "hub: create {}: {}",
                current.display(),
                e
            ))
        })?;
        let tmp_path = current.join("index.json.tmp");
        let final_path = current.join("index.json");
        fs::write(&tmp_path, &bytes).map_err(|e| {
            AppError::internal_error(format!(
                "hub: write tmp index.json: {}",
                e
            ))
        })?;
        fs::rename(&tmp_path, &final_path).map_err(|e| {
            AppError::internal_error(format!(
                "hub: promote tmp index.json: {}",
                e
            ))
        })?;
        let _ = fs::remove_file(current.join(SEED_MARKER));

        let new_version = catalog.hub_version;
        Ok(RefreshOutcome {
            updated: previous_version.as_deref() != Some(new_version.as_str()),
            previous_version,
            new_version,
            refreshed_at: Utc::now(),
        })
    }

    /// Fetch one per-entry manifest from Pages and cache it on disk
    /// at `current/<rel>`. Caller has already validated `rel`.
    async fn fetch_and_cache_manifest(&self, rel: &str) -> Result<Vec<u8>, AppError> {
        let base = hub_pages_base();
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            rel.trim_start_matches('/')
        );
        let url_owned = url.clone();
        let bytes = tokio::task::spawn_blocking(move || download_json(&url_owned))
            .await
            .map_err(|e| {
                AppError::internal_error(format!("hub: fetch-manifest join: {}", e))
            })??;

        let cache_path = self.current_dir().join(rel);
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::internal_error(format!(
                    "hub: create cache dir {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }
        let tmp_path = cache_path.with_extension("json.tmp");
        fs::write(&tmp_path, &bytes).map_err(|e| {
            AppError::internal_error(format!(
                "hub: write tmp manifest {}: {}",
                tmp_path.display(),
                e
            ))
        })?;
        fs::rename(&tmp_path, &cache_path).map_err(|e| {
            AppError::internal_error(format!(
                "hub: promote tmp manifest {}: {}",
                cache_path.display(),
                e
            ))
        })?;
        Ok(bytes)
    }

    // ----- helpers -----

    /// Copy an `include_dir::Dir` recursively onto disk.
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
// HTTP helpers (blocking — used inside spawn_blocking)
// =====================================================================

/// Synchronously GET a JSON payload from `url` with size + timeout
/// caps. Returns the body bytes on success. The size cap protects
/// against an upstream redirect that fills memory; the timeout
/// protects against a hung server.
fn download_json(url: &str) -> Result<Vec<u8>, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("ziee/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::internal_error(format!("hub: http client: {}", e)))?;

    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| AppError::internal_error(format!("hub: GET {}: {}", url, e)))?
        .error_for_status()
        .map_err(|e| AppError::internal_error(format!("hub: GET {}: {}", url, e)))?;

    if let Some(len) = resp.content_length()
        && len > MAX_HUB_ARTIFACT_BYTES
    {
        return Err(AppError::internal_error(format!(
            "hub: {} declares {} bytes (cap {})",
            url, len, MAX_HUB_ARTIFACT_BYTES
        )));
    }

    // Read with a cap — even if Content-Length is absent or lies, we
    // stop reading at the cap.
    use std::io::Read;
    let mut reader = resp.take(MAX_HUB_ARTIFACT_BYTES + 1);
    let mut buf = Vec::with_capacity(64 * 1024);
    reader
        .read_to_end(&mut buf)
        .map_err(|e| AppError::internal_error(format!("hub: read {}: {}", url, e)))?;
    if buf.len() as u64 > MAX_HUB_ARTIFACT_BYTES {
        return Err(AppError::internal_error(format!(
            "hub: {} exceeded {} bytes",
            url, MAX_HUB_ARTIFACT_BYTES
        )));
    }
    Ok(buf)
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

/// Validate a hub entry's reverse-DNS `name` before any catalog
/// lookup. Reverse-DNS strings have the shape `<namespace>/<leaf>`
/// where the namespace contains dots (`io.github.modelcontextprotocol`)
/// and the leaf is a lowercase slug. Must have exactly one `/`.
/// Conservative on length (128 chars) and rejects `..` / leading
/// dot / non-ASCII so the value is safe to log + use as a HashMap key
/// without further escaping. The on-disk path safety check is still
/// `is_safe_manifest_path` against the IndexItem's `manifest_path`.
pub(crate) fn is_safe_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    if name.contains("..") {
        return false;
    }
    let slash_count = name.bytes().filter(|&b| b == b'/').count();
    if slash_count != 1 {
        return false;
    }
    let (ns, leaf) = match name.split_once('/') {
        Some(p) => p,
        None => return false,
    };
    let ns_ok = !ns.is_empty()
        && !ns.starts_with('.')
        && !ns.ends_with('.')
        && ns.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'.'
                || b == b'-'
        });
    let leaf_ok = !leaf.is_empty()
        && !leaf.starts_with('.')
        && leaf.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'.'
                || b == b'-'
                || b == b'_'
        });
    ns_ok && leaf_ok
}

/// Derive the slug we use for the user's installed
/// `mcp_servers.name` row from a reverse-DNS `name` like
/// `io.github.modelcontextprotocol/filesystem`. Returns the leaf
/// after the FIRST `/`, lowercased, with any non-`[a-z0-9-]`
/// character collapsed to `-` and consecutive `-` runs collapsed.
/// Max 63 chars. Empty input or empty leaf returns the empty
/// string (caller treats that as a fall back to the full name).
///
/// Examples:
/// - `io.github.modelcontextprotocol/filesystem` → `filesystem`
/// - `io.github.modelcontextprotocol/server-postgres` → `server-postgres`
/// - `com.example/MyServer.v2` → `myserver-v2`
pub fn derive_mcp_slug(name: &str) -> String {
    let leaf = match name.split_once('/') {
        Some((_, after)) => after,
        None => name,
    };
    let mut out = String::with_capacity(leaf.len());
    let mut last_was_dash = false;
    for c in leaf.chars() {
        let cl = c.to_ascii_lowercase();
        if cl.is_ascii_lowercase() || cl.is_ascii_digit() {
            out.push(cl);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let limited = if trimmed.len() > 63 {
        &trimmed[..63]
    } else {
        trimmed
    };
    limited.trim_matches('-').to_string()
}

/// Validate a per-entry manifest_path before it's used as either an
/// HTTP suffix or a filesystem path under `current/`. Reject anything
/// containing `..`, an absolute prefix, a Windows root, or characters
/// outside the safe charset. Must end with `.json`. Must start with
/// one of the known category folders so a poisoned index can't read
/// arbitrary cache subtrees.
pub(crate) fn is_safe_manifest_path(rel: &str) -> bool {
    if rel.is_empty() || rel.len() > 256 {
        return false;
    }
    if !rel.ends_with(".json") {
        return false;
    }
    // No absolute / parent-dir / Windows-root.
    let path = std::path::Path::new(rel);
    if path.is_absolute() {
        return false;
    }
    for c in path.components() {
        match c {
            std::path::Component::Normal(_) => {}
            _ => return false,
        }
    }
    // Must start with a known category folder so a poisoned index
    // can't make us cache `..weird/path.json` somewhere outside the
    // expected subtree.
    rel.starts_with("models/")
        || rel.starts_with("assistants/")
        || rel.starts_with("mcp-servers/")
}

/// Guard a semver-shaped string. Kept from v1 for handlers that read
/// per-entry `version` fields from the catalog before using them in
/// a path or downstream identifier.
pub(crate) fn is_safe_version(v: &str) -> bool {
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

    fn ix(name: &str, min: Option<&str>) -> IndexItem {
        // `name` may be a bare slug (used for compat tests) or a
        // reverse-DNS string. The manifest_path is built from it
        // verbatim — these unit tests don't actually open files.
        IndexItem {
            name: name.to_string(),
            category: HubCategory::Model,
            title: None,
            summary: String::new(),
            tags: vec![],
            verified: false,
            added_at: None,
            min_ziee_version: min.map(String::from),
            manifest_path: format!("models/{name}/1.0.0.json"),
            version: None,
            meta: None,
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
        assert_eq!(
            HubManager::compat_for_server(&ix("foo", Some("not-a-version")), "0.1.0"),
            Compat::Ok
        );
    }

    #[test]
    fn is_safe_name_accepts_reverse_dns_rejects_traversal() {
        // Valid reverse-DNS shapes (namespace `/` leaf).
        assert!(is_safe_name("io.github.modelcontextprotocol/filesystem"));
        assert!(is_safe_name("io.github.phibya/llama-3-1-8b-instruct"));
        assert!(is_safe_name("com.example/foo_bar"));
        // Bare slugs (no `/`) are not valid reverse-DNS names.
        assert!(!is_safe_name("llama-3-1-8b-instruct"));
        assert!(!is_safe_name("foo.bar"));
        // Multiple slashes are not allowed.
        assert!(!is_safe_name("a/b/c"));
        // Parent-dir component is rejected.
        assert!(!is_safe_name("io.github.foo/../etc/passwd"));
        // Hidden leaf / empty / too long.
        assert!(!is_safe_name("io.github.foo/.hidden"));
        assert!(!is_safe_name(""));
        assert!(!is_safe_name(&"a".repeat(129)));
    }

    #[test]
    fn derive_mcp_slug_normalizes() {
        assert_eq!(
            derive_mcp_slug("io.github.modelcontextprotocol/filesystem"),
            "filesystem"
        );
        assert_eq!(
            derive_mcp_slug("io.github.modelcontextprotocol/server-postgres"),
            "server-postgres"
        );
        assert_eq!(derive_mcp_slug("com.example/MyServer.v2"), "myserver-v2");
        assert_eq!(derive_mcp_slug("io.github.foo/A_B C"), "a-b-c");
        // No `/` → take input as-is + normalize.
        assert_eq!(derive_mcp_slug("Just A Slug"), "just-a-slug");
    }

    #[test]
    fn is_safe_version_accepts_semver_rejects_injection() {
        assert!(is_safe_version("0.0.2"));
        assert!(is_safe_version("1.2.3-alpha.1"));
        assert!(is_safe_version("0.0.1-alpha"));
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
    fn is_safe_manifest_path_validation() {
        assert!(is_safe_manifest_path("models/llama/1.0.0.json"));
        assert!(is_safe_manifest_path("assistants/foo/1.0.0.json"));
        assert!(is_safe_manifest_path("mcp-servers/bar/2.3.4.json"));
        // Wrong extension.
        assert!(!is_safe_manifest_path("models/foo/1.0.0.yaml"));
        // Parent-dir.
        assert!(!is_safe_manifest_path("models/../etc/passwd.json"));
        // Absolute.
        assert!(!is_safe_manifest_path("/models/foo/1.0.0.json"));
        // Unknown category folder.
        assert!(!is_safe_manifest_path("evil/foo/1.0.0.json"));
        // Empty.
        assert!(!is_safe_manifest_path(""));
        // Too long.
        let huge = format!("models/{}/1.0.0.json", "a".repeat(300));
        assert!(!is_safe_manifest_path(&huge));
    }

    #[test]
    fn category_folder_is_stable() {
        assert_eq!(category_folder(HubCategory::Model), "models");
        assert_eq!(category_folder(HubCategory::Assistant), "assistants");
        assert_eq!(category_folder(HubCategory::McpServer), "mcp-servers");
    }

    #[test]
    fn server_version_matches_pkg_version() {
        assert!(semver::Version::parse(server_version()).is_ok());
    }

    #[test]
    fn seed_manifest_json_round_trips_into_structs() {
        // Pull a real manifest out of the embedded seed and parse it
        // into the typed struct — guards the JSON field mapping (the
        // manifests are authored in resources/hub-seed/, consumed here).
        // v2 path layout: `<category>/<namespace>/<leaf>/<version>.json`.
        // The seed is a snapshot of ziee-ai/hub's build output, which
        // uses the `io.github.phibya/...` namespace for ziee-native
        // entries.
        let model_json = HUB_SEED
            .get_file("models/io.github.phibya/llama-3-1-8b-instruct/1.0.0.json")
            .expect("seed has llama model");
        let model: HubModel =
            serde_json::from_slice(model_json.contents()).expect("parse model json");
        assert_eq!(model.name, "io.github.phibya/llama-3-1-8b-instruct");

        let asst_json = HUB_SEED
            .get_file("assistants/io.github.phibya/code-reviewer/1.0.0.json")
            .expect("seed has code-reviewer");
        let asst: HubAssistant = serde_json::from_slice(asst_json.contents())
            .expect("parse assistant json");
        assert_eq!(asst.name, "io.github.phibya/code-reviewer");

        let mcp_json = HUB_SEED
            .get_file("mcp-servers/io.github.github/mcp/1.0.0.json")
            .expect("seed has github mcp");
        let mcp: HubMCPServer =
            serde_json::from_slice(mcp_json.contents()).expect("parse mcp json");
        assert_eq!(mcp.name, "io.github.github/mcp");
        // The github seed entry uses remotes[] (streamable-http).
        assert!(mcp.remotes.as_ref().is_some_and(|r| !r.is_empty()));
    }

    #[test]
    fn seed_directory_carries_index_and_categories() {
        let names: Vec<_> = HUB_SEED
            .entries()
            .iter()
            .filter_map(|e| e.path().file_name().and_then(|s| s.to_str()))
            .collect();
        assert!(
            names.contains(&"index.json"),
            "seed missing index.json: {names:?}"
        );
        assert!(names.contains(&"models"));
        assert!(names.contains(&"assistants"));
        assert!(names.contains(&"mcp-servers"));
    }

    // ─────────────────────────────────────────────────────────────────
    // initialize() — seed-upgrade-on-boot matrix
    // ─────────────────────────────────────────────────────────────────

    fn fixture_with_existing_catalog(version: &str, seed_provenance: bool) -> PathBuf {
        let unique = format!(
            "hub-init-{}-{}",
            std::process::id(),
            version.replace('.', "_"),
        );
        let data_dir = std::env::temp_dir().join(format!("{unique}-{seed_provenance}"));
        let current = data_dir.join("hub").join("current");
        fs::create_dir_all(&current).unwrap();
        let body = serde_json::json!({
            "hub_version": version,
            "generated_at": "1970-01-01T00:00:00Z",
            "schema_version": 2,
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
        let dir = fixture_with_existing_catalog("0.0.0-alpha", true);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(
            cached_version(&dir),
            SEED_HUB_VERSION.trim(),
            "stale seed cache should be upgraded to the embedded seed version"
        );
        assert!(dir.join("hub").join("current").join(SEED_MARKER).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_leaves_same_version_seed_cache_alone() {
        let v = SEED_HUB_VERSION.trim();
        let dir = fixture_with_existing_catalog(v, true);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(cached_version(&dir), v);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_leaves_pages_fetched_cache_alone_even_when_older() {
        // No `.seed` marker → provenance is Pages. Operator-managed
        // catalog must NEVER be auto-replaced by the embedded seed.
        let dir = fixture_with_existing_catalog("0.0.0-alpha", /* seed */ false);
        let mgr = HubManager::new(&dir).unwrap();
        mgr.initialize().unwrap();

        assert_eq!(
            cached_version(&dir),
            "0.0.0-alpha",
            "Pages-fetched cache must not be silently rewritten by the seed"
        );
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
        let index = HUB_SEED
            .get_file("index.json")
            .expect("seed has index.json");
        let catalog: Catalog =
            serde_json::from_slice(index.contents()).expect("parse seed index.json");
        assert_eq!(
            catalog.hub_version,
            SEED_HUB_VERSION.trim(),
            "resources/hub-seed/index.json hub_version ({}) != SEED_HUB_VERSION const ({})",
            catalog.hub_version,
            SEED_HUB_VERSION
        );
    }
}
