//! HubManager — GitHub-Releases-backed catalog of models, assistants, MCP servers.
//!
//! Source of truth: `ziee-ai/hub` repo, tagged + signed releases. At
//! `tag → release.yml → ziee-ai/hub` produces `hub.tar.gz` (flat bundle of
//! manifests + schemas + index) plus `hub.index.json`, each with a
//! `.sha256` and a keyless cosign `.cosign.bundle` sidecar.
//!
//! On boot the server installs an embedded seed catalog (compiled via
//! `include_dir!` from `resources/hub-seed/`, sourced from `v0.0.1-alpha`)
//! so the hub UI renders read-only even when GitHub is unreachable.
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
// Embedded seed (from v0.0.1-alpha, sourced at build time)
// =====================================================================

static HUB_SEED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/resources/hub-seed");

/// Seed catalog version — kept in sync with the directory under
/// `resources/hub-seed/` at build time. Bumped whenever a new seed is
/// staged for a release.
pub const SEED_HUB_VERSION: &str = "0.0.1-alpha";

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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum HubManifest {
    Model(Box<HubModel>),
    Assistant(Box<HubAssistant>),
    McpServer(Box<HubMCPServer>),
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
        self.app_data_dir.join("hub")
    }

    fn current_dir(&self) -> PathBuf {
        self.hub_root().join("current")
    }

    fn staging_dir(&self) -> PathBuf {
        self.hub_root().join(".staging")
    }

    /// On boot: install the embedded seed catalog into
    /// `<app_data>/hub/current/` if it doesn't already exist. Idempotent.
    pub fn initialize(&self) -> Result<(), AppError> {
        let current = self.current_dir();
        if current.join("index.json").exists() {
            // Already initialized (either by a previous boot or by a
            // successful refresh from GitHub). Don't clobber.
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
        tracing::info!(
            "hub: installed embedded seed catalog v{} into {}",
            SEED_HUB_VERSION,
            current.display()
        );
        Ok(())
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
                Ok(HubManifest::Model(Box::new(m)))
            }
            HubCategory::Assistant => {
                let a: HubAssistant = serde_yaml::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse assistant {}: {}",
                        id, e
                    ))
                })?;
                Ok(HubManifest::Assistant(Box::new(a)))
            }
            HubCategory::McpServer => {
                let s: HubMCPServer = serde_yaml::from_slice(&bytes).map_err(|e| {
                    AppError::internal_error(format!(
                        "hub: parse mcp-server {}: {}",
                        id, e
                    ))
                })?;
                Ok(HubManifest::McpServer(Box::new(s)))
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
            if let HubManifest::Model(m) = self.manifest(item.category, &item.id).await? {
                out.push(*m);
            }
        }
        Ok(out)
    }

    pub async fn list_assistants(&self) -> Result<Vec<HubAssistant>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog.items.iter().filter(|i| matches!(i.category, HubCategory::Assistant)) {
            if let HubManifest::Assistant(a) = self.manifest(item.category, &item.id).await? {
                out.push(*a);
            }
        }
        Ok(out)
    }

    pub async fn list_mcp_servers(&self) -> Result<Vec<HubMCPServer>, AppError> {
        let catalog = self.catalog().await?;
        let mut out = Vec::new();
        for item in catalog.items.iter().filter(|i| matches!(i.category, HubCategory::McpServer)) {
            if let HubManifest::McpServer(s) = self.manifest(item.category, &item.id).await? {
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

    /// Read the catalog's `hub_version` without parsing the whole index.
    pub async fn get_current_version(&self, _category: &str) -> Result<String, AppError> {
        let catalog = self.catalog().await?;
        Ok(catalog.hub_version)
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

    /// Back-compat shim for the old per-category refresh endpoints
    /// (`/api/hub/{models,assistants,mcp-servers}/refresh`). The new
    /// catalog model refreshes everything in one shot, so the
    /// `_category` arg is accepted but ignored — each per-category
    /// endpoint triggers the same full refresh, then emits its own
    /// category-specific event for any consumers wired to one.
    pub async fn refresh_hub_category(&self, _category: &str) -> Result<(), AppError> {
        self.refresh().await.map(|_| ())
    }

    /// Force-refresh the catalog from GitHub Releases. Returns the
    /// previous version (None on first refresh after install) and the
    /// new version. Cosign + sha256 failure aborts; the previous
    /// `current/` is left untouched.
    pub async fn refresh(&self) -> Result<RefreshOutcome, AppError> {
        let previous_version = self
            .catalog()
            .await
            .ok()
            .map(|c| c.hub_version);

        let app_data = self.app_data_dir.clone();
        let outcome = tokio::task::spawn_blocking(move || refresh_blocking(&app_data))
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
// `compat()` static helper — accessible without constructing HubManager
// (used by handlers gating /install on category at request time).
// =====================================================================

pub fn compat_of(item: &IndexItem) -> Compat {
    HubManager::compat(item)
}

// =====================================================================
// Refresh path (blocking — runs on spawn_blocking worker thread)
// =====================================================================

struct BlockingOutcome {
    new_version: String,
    cosign_verified: bool,
}

fn refresh_blocking(app_data: &Path) -> Result<BlockingOutcome, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("ziee-chat/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::internal_error(format!("hub: http client: {}", e)))?;

    let latest = resolve_latest_release(&client)?;
    let tag = latest.tag_name.clone();

    let staging = app_data.join("hub").join(".staging");
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
    for asset in assets {
        let url = format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            HUB_REPO_OWNER, HUB_REPO_NAME, tag, asset
        );
        download_to_file(&client, &url, &staging.join(asset))?;
    }

    let tar_path = staging.join("hub.tar.gz");
    let index_path = staging.join("hub.index.json");

    // sha256 both.
    verify_sha256_sidecar(&tar_path, &staging.join("hub.tar.gz.sha256"))?;
    verify_sha256_sidecar(&index_path, &staging.join("hub.index.json.sha256"))?;

    // cosign keyless both, fail-closed (no signed=false fallback).
    let identity = cosign_expected_identity(&tag);
    let cosign_verified = match (
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
    let current = app_data.join("hub").join("current");
    let backup = app_data.join("hub").join(".previous");
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
}

fn resolve_latest_release(client: &reqwest::blocking::Client) -> Result<GhRelease, AppError> {
    // List recent releases — `/releases/latest` skips prereleases by
    // definition, but we still need to surface them when stable hasn't
    // shipped yet (e.g. during the v0.0.x-alpha window). Strategy:
    // prefer the most recent non-prerelease tag; fall back to the
    // newest prerelease if no stable exists.
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=20",
        HUB_REPO_OWNER, HUB_REPO_NAME
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
    for entry in archive.entries().map_err(|e| {
        AppError::internal_error(format!("hub: read archive: {}", e))
    })? {
        let mut entry = entry.map_err(|e| {
            AppError::internal_error(format!("hub: read entry: {}", e))
        })?;
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
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(AppError::internal_error(format!(
                    "hub: refusing parent-dir component in archive: {}",
                    path.display()
                )));
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
}
