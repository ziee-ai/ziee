//! Downloader for the `whisper-server` binary from GitHub releases.
//!
//! Adapted from `llm_local_runtime::engine::download`, scoped to the SINGLE
//! whisper engine (`ziee-ai/whisper.cpp` fork). Differences from the LLM
//! downloader:
//! - No `EngineType` — the repo slug + binary name are fixed constants.
//! - Cache layout is `whisper-runtime/binaries/<version>/<platform>-<arch>-<backend>/`
//!   under the app data dir (no engine segment).
//! - The `.sha256` sidecar is **MANDATORY** (like `build_helper/biomcp.rs`): a
//!   missing / malformed / mismatching digest FAILS the install rather than
//!   proceeding TOFU. The whisper fork CI publishes a `<asset>.sha256` next to
//!   every artifact.
//!
//! Shared with the LLM downloader: the `error`/`binary` helper modules are
//! reused directly (`RuntimeError`/`Result`, `ensure_executable`) so the two
//! runtimes agree on error taxonomy + Unix exec-bit handling.

use crate::modules::llm_local_runtime::engine::binary::ensure_executable;
use crate::modules::llm_local_runtime::engine::error::{Result, RuntimeError};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// GitHub repo slug for the whisper.cpp fork whose CI publishes prebuilt
/// `whisper-server` binaries + `.sha256` sidecars.
const WHISPER_REPO: &str = "ziee-ai/whisper.cpp";

/// Base host for release-artifact downloads.
///
/// Defaults to `https://github.com`. In **debug builds only** the
/// `WHISPER_RUNTIME_RELEASE_MIRROR` env var may override it so integration
/// tests can serve a stub artifact from a loopback mock release server (mirrors
/// the LLM runtime's `LLM_RUNTIME_RELEASE_MIRROR`). The env read is compiled out
/// of release builds via `cfg!(debug_assertions)`, so the production binary
/// always points at the real GitHub host.
fn release_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("WHISPER_RUNTIME_RELEASE_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://github.com".to_string()
}

/// Base host for the GitHub API (used to resolve `latest` → a tag + list
/// releases). Defaults to `https://api.github.com`; debug-only override via
/// `WHISPER_RUNTIME_API_MIRROR`. Same compile-out rules as [`release_base_url`].
fn api_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("WHISPER_RUNTIME_API_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://api.github.com".to_string()
}

/// The binary name *inside* a release archive (`.exe` on Windows).
fn binary_name(platform: &str) -> &'static str {
    if platform == "windows" {
        "whisper-server.exe"
    } else {
        "whisper-server"
    }
}

/// Release archive extension for a platform (`zip` on Windows, else `tar.gz`).
fn archive_ext(platform: &str) -> &'static str {
    if platform == "windows" { "zip" } else { "tar.gz" }
}

/// The release asset filename for one (platform, arch, backend):
/// `"whisper-server-{platform}-{arch}-{backend}.{ext}"`. The single source of
/// truth for both the download URL and asset-readiness detection — must match
/// the whisper.cpp fork's CI naming contract exactly.
fn archive_name(platform: &str, arch: &str, backend: &str) -> String {
    format!(
        "whisper-server-{}-{}-{}.{}",
        platform,
        arch,
        backend,
        archive_ext(platform),
    )
}

/// If `asset` is the release archive for this (platform, arch), return its
/// backend segment (e.g. `cpu`, `cuda`); else `None`.
///
/// Naturally rejects sibling `.sha256` assets (`….tar.gz.sha256` does not end
/// in `.tar.gz`) and other-arch/other-platform archives.
fn asset_backend(platform: &str, arch: &str, asset: &str) -> Option<String> {
    let prefix = format!("whisper-server-{}-{}-", platform, arch);
    let suffix = format!(".{}", archive_ext(platform));
    asset
        .strip_prefix(&prefix)?
        .strip_suffix(&suffix)
        .map(|s| s.to_string())
}

/// One release asset, reduced to what update-checking needs: the filename +
/// GitHub's reported byte size (so the UI can render the download size up-front).
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub name: String,
    pub size_bytes: u64,
}

/// Backends published for (platform, arch) given a release's assets. Empty ⇒
/// the release exists but its binary for this host is not (yet) uploaded — the
/// build-pending case.
pub fn available_backends(platform: &str, arch: &str, assets: &[AssetInfo]) -> Vec<String> {
    assets
        .iter()
        .filter_map(|a| asset_backend(platform, arch, &a.name))
        .collect()
}

/// The byte size of the host-matching binary archive for a specific backend.
/// Returns `None` when no asset matches (build-pending) or GitHub omitted the
/// `size` field.
pub fn asset_size_for_backend(
    platform: &str,
    arch: &str,
    backend: &str,
    assets: &[AssetInfo],
) -> Option<u64> {
    let target = archive_name(platform, arch, backend);
    assets.iter().find(|a| a.name == target).map(|a| a.size_bytes)
}

/// One upstream release, reduced to what update-checking needs.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// Release tag (e.g. `v0.0.1-alpha`).
    pub version: String,
    /// GitHub draft flag — drafts are not public/installable.
    pub draft: bool,
    /// GitHub prerelease flag.
    pub prerelease: bool,
    /// ISO-8601 publish timestamp, if present.
    pub published_at: Option<String>,
    /// All assets attached to the release (filename + byte size).
    pub assets: Vec<AssetInfo>,
}

/// GitHub binary downloader for `whisper-server`.
pub struct WhisperDownloader {
    binaries_dir: PathBuf,
    client: reqwest::Client,
}

/// Information about a downloaded binary.
#[derive(Debug, Clone)]
pub struct BinaryInfo {
    /// Version tag (e.g., "v0.7.0").
    pub version: String,
    /// Platform (e.g., "linux", "macos", "windows").
    pub platform: String,
    /// Architecture (e.g., "x86_64", "aarch64").
    pub arch: String,
    /// Backend (e.g., "cpu", "cuda", "metal").
    pub backend: String,
    /// Local path to the binary.
    pub path: PathBuf,
    /// File size in bytes.
    #[allow(dead_code)]
    pub size_bytes: u64,
}

impl WhisperDownloader {
    /// Create a downloader rooted at the default cache directory:
    /// `<app_data>/whisper-runtime/binaries/`.
    pub fn new() -> Result<Self> {
        Self::with_binaries_dir(Self::default_binaries_dir())
    }

    /// Create a downloader with a custom binaries directory.
    pub fn with_binaries_dir(binaries_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&binaries_dir)?;

        let client = reqwest::Client::builder()
            .user_agent("ziee-whisper-runtime/0.1.0")
            // Cap connection setup and per-read inactivity so a stalled peer
            // can't hang the transfer forever. No blanket request timeout —
            // large binary downloads are legitimately long-running; the
            // read_timeout only fires on no-progress.
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()?;

        Ok(Self {
            binaries_dir,
            client,
        })
    }

    /// The default binaries directory: `<app_data>/whisper-runtime/binaries/`.
    fn default_binaries_dir() -> PathBuf {
        crate::core::get_app_data_dir()
            .join("whisper-runtime")
            .join("binaries")
    }

    /// The cache dir for one (version, platform, arch, backend):
    /// `<binaries_dir>/<version>/<platform>-<arch>-<backend>/`.
    fn cache_dir(&self, version: &str, platform: &str, arch: &str, backend: &str) -> PathBuf {
        self.binaries_dir
            .join(version)
            .join(format!("{}-{}-{}", platform, arch, backend))
    }

    /// Download the `whisper-server` binary from GitHub releases with a
    /// per-chunk progress callback `(bytes_received_so_far, total_bytes)`.
    /// `total_bytes` is `None` when the upstream omits Content-Length.
    ///
    /// Verifies the artifact against its mandatory `.sha256` sidecar before
    /// extraction. Skips the network entirely when the binary is already cached.
    pub async fn download_with_progress<F>(
        &self,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
        progress: F,
    ) -> Result<BinaryInfo>
    where
        F: Fn(u64, Option<u64>) + Send + Sync,
    {
        let bin_name = binary_name(platform);
        let archive_name = archive_name(platform, arch, backend);

        // Resolve version if "latest".
        let resolved_version = if version == "latest" {
            self.get_latest_version().await?
        } else {
            version.to_string()
        };

        tracing::info!(
            "Downloading whisper-server {} for {}-{}-{}",
            resolved_version,
            platform,
            arch,
            backend
        );

        // Check if already cached.
        let cache_dir = self.cache_dir(&resolved_version, platform, arch, backend);
        let binary_path = cache_dir.join(bin_name);

        if binary_path.exists() {
            tracing::info!("whisper-server already cached: {}", binary_path.display());
            let metadata = std::fs::metadata(&binary_path)?;
            #[cfg(unix)]
            ensure_executable(&binary_path)?;
            return Ok(BinaryInfo {
                version: resolved_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
                backend: backend.to_string(),
                path: binary_path,
                size_bytes: metadata.len(),
            });
        }

        // Construct the GitHub release URL (host overridable in debug builds).
        let download_url = format!(
            "{}/{}/releases/download/{}/{}",
            release_base_url(),
            WHISPER_REPO,
            resolved_version,
            archive_name
        );
        tracing::info!("Downloading from: {}", download_url);

        // Temp download dir.
        let temp_dir = self.binaries_dir.join(".tmp");
        std::fs::create_dir_all(&temp_dir)?;
        let temp_archive = temp_dir.join(&archive_name);

        // Download the archive. A 404 here is the automated-release race: the
        // tag can exist before CI finishes building + uploading the
        // per-platform binary, so a fetch that misses means "build pending".
        self.download_file(&download_url, &temp_archive, Some(&progress))
            .await
            .map_err(|e| {
                RuntimeError::BinaryNotFound(format!(
                    "whisper-server binary not published for {resolved_version} \
                     {platform}/{arch}/{backend} ({archive_name}): {e}. If the \
                     release was just created, its CI build may still be in \
                     progress — retry later."
                ))
            })?;

        // MANDATORY sha256 verify against the `<asset>.sha256` sidecar. A
        // missing / malformed / mismatching digest is a hard failure — we
        // never install an unverified binary (supply-chain defense, mirrors
        // build_helper/biomcp.rs). Clean up the partial archive on any failure.
        if let Err(e) = self.verify_sha256(&download_url, &temp_archive).await {
            let _ = std::fs::remove_file(&temp_archive);
            return Err(e);
        }

        // Extract binary from archive.
        std::fs::create_dir_all(&cache_dir)?;
        let extract_result = if platform == "windows" {
            self.extract_zip(&temp_archive, &cache_dir, bin_name)
        } else {
            self.extract_tar_gz(&temp_archive, &cache_dir, bin_name)
        };

        // Always clean up the temp archive, even on extract failure.
        let _ = std::fs::remove_file(&temp_archive);
        // On ANY extract error, remove the partially-populated cache dir. The
        // binary (an early entry) can already be on disk when a later entry
        // fails; leaving it would make a retry short-circuit on
        // `binary_path.exists()` above and return an incomplete install.
        if let Err(e) = extract_result {
            let _ = std::fs::remove_dir_all(&cache_dir);
            return Err(e);
        }

        #[cfg(unix)]
        ensure_executable(&binary_path)?;

        let metadata = std::fs::metadata(&binary_path)?;
        tracing::info!("whisper-server downloaded: {}", binary_path.display());

        Ok(BinaryInfo {
            version: resolved_version,
            platform: platform.to_string(),
            arch: arch.to_string(),
            backend: backend.to_string(),
            path: binary_path,
            size_bytes: metadata.len(),
        })
    }

    /// Fetch the `<url>.sha256` sidecar and verify it against the file at
    /// `archive_path`. Mandatory: a missing / malformed / mismatching digest
    /// returns an error.
    async fn verify_sha256(&self, download_url: &str, archive_path: &Path) -> Result<()> {
        let sha_url = format!("{}.sha256", download_url);
        let resp = self
            .client
            .get(&sha_url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(RuntimeError::network(format!(
                "sha256 sidecar unavailable ({}): refusing to install an unverified \
                 whisper-server binary",
                resp.status()
            )));
        }
        let text = resp.text().await?;
        // Sidecar format is `<64-hex>  <filename>`; take the first token.
        let expected = text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(RuntimeError::internal(format!(
                "sha256 sidecar malformed (not 64 hex chars): {:?}",
                expected
            )));
        }

        let actual = sha256_of_file(archive_path)?;
        if actual != expected {
            return Err(RuntimeError::internal(format!(
                "sha256 mismatch for whisper-server archive: expected {}, got {}",
                expected, actual
            )));
        }
        tracing::info!("whisper-server sha256 verified.");
        Ok(())
    }

    /// GET a GitHub API URL with exponential-backoff retry on transient
    /// failures (network/timeout, HTTP 5xx, 429). A single hiccup on the
    /// release-resolution path shouldn't fail the whole download/version-check.
    async fn github_get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let result = self
                .client
                .get(url)
                .header("Accept", "application/vnd.github.v3+json")
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await;
            let transient = match &result {
                Ok(resp) => resp.status().is_server_error() || resp.status().as_u16() == 429,
                Err(_) => true,
            };
            if transient && attempt < MAX_ATTEMPTS {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::warn!(
                    "GitHub API {url}: transient failure, retrying in {delay:?} (attempt {attempt}/{MAX_ATTEMPTS})"
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            return Ok(result?);
        }
    }

    /// Get the latest release tag from GitHub.
    pub async fn get_latest_version(&self) -> Result<String> {
        let url = format!("{}/repos/{}/releases/latest", api_base_url(), WHISPER_REPO);
        let response = self.github_get_with_retry(&url).await?;
        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to get latest whisper-server release: HTTP {}",
                response.status()
            )));
        }
        let json: serde_json::Value = response.json().await?;
        let tag_name = json["tag_name"]
            .as_str()
            .ok_or_else(|| RuntimeError::network("Could not parse latest release tag"))?;
        Ok(tag_name.to_string())
    }

    /// List the whisper fork's upstream releases (newest first), each reduced
    /// to a [`ReleaseInfo`]. Mirror-aware via [`api_base_url`].
    pub async fn list_releases(&self) -> Result<Vec<ReleaseInfo>> {
        let url = format!("{}/repos/{}/releases", api_base_url(), WHISPER_REPO);
        let response = self.github_get_with_retry(&url).await?;
        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to list whisper-server releases: HTTP {}",
                response.status()
            )));
        }
        let releases: Vec<serde_json::Value> = response.json().await?;
        Ok(releases
            .iter()
            .filter_map(|r| {
                let version = r["tag_name"].as_str()?.to_string();
                let assets = r["assets"]
                    .as_array()
                    .map(|assets| {
                        assets
                            .iter()
                            .filter_map(|a| {
                                let name = a["name"].as_str()?.to_string();
                                let size_bytes = a["size"].as_u64().unwrap_or(0);
                                Some(AssetInfo { name, size_bytes })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some(ReleaseInfo {
                    version,
                    draft: r["draft"].as_bool().unwrap_or(false),
                    prerelease: r["prerelease"].as_bool().unwrap_or(false),
                    published_at: r["published_at"].as_str().map(String::from),
                    assets,
                })
            })
            .collect())
    }

    /// Download a file with a chunked progress callback.
    ///
    /// Enforces a 2 GiB hard cap on downloaded bytes so an attacker-controlled
    /// upstream (e.g. a hijacked mirror redirecting to a /dev/zero stream)
    /// can't fill the host disk. Cryptographic verification happens separately
    /// via [`WhisperDownloader::verify_sha256`].
    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: Option<&(dyn Fn(u64, Option<u64>) + Send + Sync)>,
    ) -> Result<()> {
        const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

        // Pre-check Content-Length via HEAD when available; fail fast.
        let head_response = self.client.head(url).send().await?;
        if !head_response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to access file: HTTP {}",
                head_response.status()
            )));
        }
        let total_size = head_response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        if total_size > MAX_DOWNLOAD_BYTES {
            return Err(RuntimeError::network(format!(
                "Refusing to download {} bytes (cap {} bytes / 2 GiB)",
                total_size, MAX_DOWNLOAD_BYTES
            )));
        }

        tracing::debug!(
            "Downloading {} ({} bytes)",
            dest.file_name().unwrap().to_string_lossy(),
            total_size
        );

        let mut response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to download: HTTP {}",
                response.status()
            )));
        }

        let mut file = File::create(dest)?;
        let mut received: u64 = 0;
        let total_for_cb = if total_size > 0 { Some(total_size) } else { None };
        if let Some(cb) = progress {
            cb(0, total_for_cb);
        }

        while let Some(chunk) = response.chunk().await? {
            received = received.saturating_add(chunk.len() as u64);
            if received > MAX_DOWNLOAD_BYTES {
                let _ = std::fs::remove_file(dest);
                return Err(RuntimeError::network(format!(
                    "Download exceeded {} bytes / 2 GiB cap; aborted",
                    MAX_DOWNLOAD_BYTES
                )));
            }
            file.write_all(&chunk)?;
            if let Some(cb) = progress {
                cb(received, total_for_cb);
            }
        }

        tracing::debug!("Downloaded {}", dest.file_name().unwrap().to_string_lossy());
        Ok(())
    }

    /// Extract the binary + all shared libraries from a tar.gz archive.
    /// Flattens every entry into `dest_dir`; recreates only same-dir library
    /// SONAME symlinks (rejecting any escaping symlink — path-traversal guard).
    fn extract_tar_gz(&self, archive: &Path, dest_dir: &Path, bin_name: &str) -> Result<()> {
        let tar_gz = File::open(archive)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.set_preserve_permissions(true);
        archive.set_preserve_mtime(true);
        archive.set_unpack_xattrs(true);

        let mut binary_found = false;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_type = entry.header().entry_type();
            if entry_type.is_dir() {
                continue;
            }

            let file_name = entry
                .path()
                .ok()
                .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                .unwrap_or_default();
            if file_name.is_empty() {
                continue;
            }

            if entry_type.is_hard_link() {
                tracing::warn!("Skipping hardlink entry in archive: {}", file_name);
                continue;
            }

            let is_library = file_name.ends_with(".so")
                || file_name.contains(".so.")
                || file_name.ends_with(".dylib")
                || file_name.ends_with(".dll");

            if entry_type.is_symlink() {
                let link: Option<PathBuf> =
                    entry.link_name().ok().flatten().map(|c| c.into_owned());
                match link.as_deref().and_then(safe_same_dir_symlink_target) {
                    Some(target) if is_library => {
                        let link_path = dest_dir.join(&file_name);
                        recreate_symlink(&target, &link_path)?;
                        tracing::debug!(
                            "Recreated library symlink: {} -> {}",
                            link_path.display(),
                            target.to_string_lossy()
                        );
                    }
                    _ => {
                        tracing::warn!(
                            "Skipping unsafe or non-library symlink entry in archive: {} -> {:?}",
                            file_name, link
                        );
                    }
                }
                continue;
            }

            if file_name == bin_name {
                let dest_path = dest_dir.join(bin_name);
                entry.unpack(&dest_path)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            if is_library {
                let dest_path = dest_dir.join(&file_name);
                entry.unpack(&dest_path)?;
                tracing::debug!("Extracted library: {}", dest_path.display());
            }
        }

        if !binary_found {
            return Err(RuntimeError::internal(format!(
                "Binary '{}' not found in archive",
                bin_name
            )));
        }
        Ok(())
    }

    /// Extract the binary + all DLLs from a zip archive.
    fn extract_zip(&self, archive: &Path, dest_dir: &Path, bin_name: &str) -> Result<()> {
        let file = File::open(archive)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut binary_found = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name();
            if name.ends_with('/') || name.ends_with('\\') {
                continue;
            }
            let file_name = std::path::Path::new(name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if file_name == bin_name {
                let dest_path = dest_dir.join(bin_name);
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }
            if file_name.ends_with(".dll") {
                let dest_path = dest_dir.join(file_name);
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                tracing::debug!("Extracted DLL: {}", dest_path.display());
            }
        }

        if !binary_found {
            return Err(RuntimeError::internal(format!(
                "Binary '{}' not found in archive",
                bin_name
            )));
        }
        Ok(())
    }

    /// List all cached whisper-server binaries by scanning the cache layout
    /// `<binaries_dir>/<version>/<platform>-<arch>-<backend>/whisper-server`.
    pub fn list_binaries(&self) -> Result<Vec<BinaryInfo>> {
        let mut binaries = Vec::new();
        if !self.binaries_dir.exists() {
            return Ok(binaries);
        }

        for version_entry in std::fs::read_dir(&self.binaries_dir)? {
            let version_entry = version_entry?;
            let version_dir = version_entry.path();
            if !version_dir.is_dir() || version_dir.file_name().unwrap() == ".tmp" {
                continue;
            }
            let version = version_dir.file_name().unwrap().to_string_lossy().to_string();

            for build_entry in std::fs::read_dir(&version_dir)? {
                let build_entry = build_entry?;
                let build_dir = build_entry.path();
                if !build_dir.is_dir() {
                    continue;
                }
                let build_name = build_dir.file_name().unwrap().to_string_lossy();
                let parts: Vec<&str> = build_name.split('-').collect();
                if parts.len() != 3 {
                    continue;
                }
                let (platform, arch, backend) = (parts[0], parts[1], parts[2]);
                let bin_name = binary_name(platform);
                let binary_path = build_dir.join(bin_name);
                if binary_path.exists() {
                    let metadata = std::fs::metadata(&binary_path)?;
                    binaries.push(BinaryInfo {
                        version: version.clone(),
                        platform: platform.to_string(),
                        arch: arch.to_string(),
                        backend: backend.to_string(),
                        path: binary_path,
                        size_bytes: metadata.len(),
                    });
                }
            }
        }
        Ok(binaries)
    }
}

/// Stream-hash a file with SHA-256, returning the lowercase hex digest.
/// Streams in 64 KiB chunks so a fat binary never needs to fit in memory.
fn sha256_of_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect())
}

/// Accept a symlink target only when it names a single entry in the SAME
/// directory (e.g. `libfoo.so.0 -> libfoo.so.0.1.2`). Returns the bare target
/// filename when safe; `None` for absolute targets, `..`, or multi-component
/// paths (escape attempts).
fn safe_same_dir_symlink_target(link: &Path) -> Option<std::ffi::OsString> {
    use std::path::Component;
    let mut comps = link.components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(name)), None) => Some(name.to_os_string()),
        _ => None,
    }
}

/// Create a relative, same-dir symlink at `link_path` pointing to `target`.
/// Unix-only; a no-op on other platforms (Windows ships DLLs as regular files).
fn recreate_symlink(target: &std::ffi::OsStr, link_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(link_path);
        std::os::unix::fs::symlink(target, link_path)
            .map_err(|e| RuntimeError::internal(format!("symlink create failed: {e}")))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link_path);
        Ok(())
    }
}

impl Default for WhisperDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default whisper downloader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base hosts default to the real GitHub endpoints when the (debug-only)
    /// mirror env vars are unset, and — in debug builds — honor the override
    /// while trimming a trailing slash so URL construction never double-slashes.
    #[test]
    fn base_urls_default_to_github_and_honor_mirror_override() {
        // SAFETY: edition-2024 marks env mutation unsafe (process global). This
        // is the only test in this module touching these vars.
        unsafe {
            std::env::remove_var("WHISPER_RUNTIME_RELEASE_MIRROR");
            std::env::remove_var("WHISPER_RUNTIME_API_MIRROR");
        }
        assert_eq!(release_base_url(), "https://github.com");
        assert_eq!(api_base_url(), "https://api.github.com");

        #[cfg(debug_assertions)]
        unsafe {
            std::env::set_var("WHISPER_RUNTIME_RELEASE_MIRROR", "http://127.0.0.1:9999/");
            assert_eq!(release_base_url(), "http://127.0.0.1:9999");
            std::env::set_var("WHISPER_RUNTIME_RELEASE_MIRROR", "");
            assert_eq!(release_base_url(), "https://github.com");
            std::env::remove_var("WHISPER_RUNTIME_RELEASE_MIRROR");
        }
    }

    #[test]
    fn archive_name_matches_fork_ci_contract() {
        assert_eq!(
            archive_name("linux", "x86_64", "cpu"),
            "whisper-server-linux-x86_64-cpu.tar.gz"
        );
        assert_eq!(
            archive_name("windows", "x86_64", "cpu"),
            "whisper-server-windows-x86_64-cpu.zip"
        );
        assert_eq!(binary_name("linux"), "whisper-server");
        assert_eq!(binary_name("windows"), "whisper-server.exe");
    }

    #[test]
    fn asset_backend_parses_and_rejects_siblings() {
        let assets = vec![
            AssetInfo { name: "whisper-server-linux-x86_64-cpu.tar.gz".into(), size_bytes: 10 },
            AssetInfo { name: "whisper-server-linux-x86_64-cpu.tar.gz.sha256".into(), size_bytes: 1 },
            AssetInfo { name: "whisper-server-macos-aarch64-metal.tar.gz".into(), size_bytes: 20 },
        ];
        // Only the matching-host archive is picked up; the `.sha256` sidecar and
        // the other-platform archive are rejected.
        assert_eq!(available_backends("linux", "x86_64", &assets), vec!["cpu".to_string()]);
        assert_eq!(
            asset_size_for_backend("linux", "x86_64", "cpu", &assets),
            Some(10)
        );
        assert_eq!(asset_size_for_backend("linux", "x86_64", "cuda", &assets), None);
    }

    #[test]
    fn safe_symlink_target_accepts_same_dir_rejects_escaping() {
        use std::path::Path;
        assert_eq!(
            safe_same_dir_symlink_target(Path::new("libfoo.so.0.1.2")).as_deref(),
            Some(std::ffi::OsStr::new("libfoo.so.0.1.2"))
        );
        assert!(safe_same_dir_symlink_target(Path::new("/etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("../../etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("sub/libfoo.so")).is_none());
    }

    #[test]
    fn sha256_of_file_matches_known_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("data.bin");
        std::fs::write(&p, b"abc").unwrap();
        // Known SHA-256 of "abc".
        assert_eq!(
            sha256_of_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
