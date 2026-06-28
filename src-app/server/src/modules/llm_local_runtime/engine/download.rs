//! Binary downloader for engine executables from GitHub releases
//!
//! Downloads pre-built engine binaries from GitHub releases with:
//! - Progress bars
//! - Resume support for interrupted downloads
//! - Automatic caching in ~/.llm-runtime/binaries/
//! - Executable permission setting (Unix)

use super::error::{Result, RuntimeError};
use super::types::EngineType;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Base host for release-artifact downloads.
///
/// Defaults to `https://github.com`. In **debug builds only** the
/// `LLM_RUNTIME_RELEASE_MIRROR` env var may override it so integration
/// tests can serve a stub engine from a loopback mock release server
/// (mirrors code_sandbox's `CODE_SANDBOX_ROOTFS_MIRROR`). The env read is
/// compiled out of release builds via `cfg!(debug_assertions)`, so the
/// production binary always points at the real GitHub host.
fn release_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("LLM_RUNTIME_RELEASE_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://github.com".to_string()
}

/// Base host for the GitHub API (used to resolve `latest` → a tag).
///
/// Defaults to `https://api.github.com`; debug-only override via
/// `LLM_RUNTIME_API_MIRROR`. Same compile-out rules as
/// [`release_base_url`]. Most tests pass an explicit version and never
/// hit this path.
fn api_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("LLM_RUNTIME_API_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://api.github.com".to_string()
}

/// GitHub repo slug for an engine's fork.
fn engine_repo(engine: EngineType) -> &'static str {
    match engine {
        EngineType::Llamacpp => "ziee-ai/llama.cpp",
        EngineType::Mistralrs => "ziee-ai/mistral.rs",
    }
}

/// The binary name *inside* a release archive (`.exe` on Windows).
fn engine_binary_name(engine: EngineType, platform: &str) -> &'static str {
    match (engine, platform == "windows") {
        (EngineType::Llamacpp, false) => "llama-server",
        (EngineType::Llamacpp, true) => "llama-server.exe",
        (EngineType::Mistralrs, false) => "mistralrs-server",
        (EngineType::Mistralrs, true) => "mistralrs-server.exe",
    }
}

/// The archive-name stem (no `.exe`) used in release asset filenames.
fn archive_stem(engine: EngineType) -> &'static str {
    match engine {
        EngineType::Llamacpp => "llama-server",
        EngineType::Mistralrs => "mistralrs-server",
    }
}

/// Release archive extension for a platform (`zip` on Windows, else `tar.gz`).
fn archive_ext(platform: &str) -> &'static str {
    if platform == "windows" { "zip" } else { "tar.gz" }
}

/// The release asset filename for one (engine, platform, arch, backend):
/// `"{stem}-{platform}-{arch}-{backend}.{ext}"`. The single source of truth
/// for both the download URL and asset-readiness detection.
fn archive_name(engine: EngineType, platform: &str, arch: &str, backend: &str) -> String {
    format!(
        "{}-{}-{}-{}.{}",
        archive_stem(engine),
        platform,
        arch,
        backend,
        archive_ext(platform),
    )
}

/// If `asset` is the release archive for this (engine, platform, arch),
/// return its backend segment (e.g. `cpu`, `cuda`); else `None`.
///
/// Naturally rejects sibling `.sig` assets (`….tar.gz.sig` does not end in
/// `.tar.gz`) and other-arch/other-platform archives.
fn asset_backend(engine: EngineType, platform: &str, arch: &str, asset: &str) -> Option<String> {
    let prefix = format!("{}-{}-{}-", archive_stem(engine), platform, arch);
    let suffix = format!(".{}", archive_ext(platform));
    asset
        .strip_prefix(&prefix)?
        .strip_suffix(&suffix)
        .map(|s| s.to_string())
}

/// One release asset, reduced to what update-checking needs:
/// the filename + GitHub's reported byte size (so the UI can render
/// the download size up-front and the user can make an informed
/// pick when CPU vs CUDA builds are very different).
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub name: String,
    pub size_bytes: u64,
}

/// Backends published for (engine, platform, arch) given a release's
/// assets. Empty ⇒ the release exists but its binary for this host
/// is not (yet) uploaded — the build-pending case.
pub fn available_backends(
    engine: EngineType,
    platform: &str,
    arch: &str,
    assets: &[AssetInfo],
) -> Vec<String> {
    assets
        .iter()
        .filter_map(|a| asset_backend(engine, platform, arch, &a.name))
        .collect()
}

/// The byte size of the host-matching binary archive for a specific
/// backend. Returns `None` when no asset matches (build-pending
/// case) or when GitHub omitted the `size` field (which it never
/// does in practice for published assets).
pub fn asset_size_for_backend(
    engine: EngineType,
    platform: &str,
    arch: &str,
    backend: &str,
    assets: &[AssetInfo],
) -> Option<u64> {
    let target = archive_name(engine, platform, arch, backend);
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

/// GitHub binary downloader
pub struct BinaryDownloader {
    binaries_dir: PathBuf,
    client: reqwest::Client,
}

/// Information about a downloaded binary
#[derive(Debug, Clone)]
pub struct BinaryInfo {
    /// Engine type
    pub engine: EngineType,

    /// Version tag (e.g., "v0.7.0")
    pub version: String,

    /// Platform (e.g., "linux", "macos", "windows")
    pub platform: String,

    /// Architecture (e.g., "x86_64", "aarch64")
    pub arch: String,

    /// Backend (e.g., "cpu", "cuda", "metal")
    pub backend: String,

    /// Local path to the binary
    pub path: PathBuf,

    /// File size in bytes
    pub size_bytes: u64,
}

impl BinaryDownloader {
    /// Create a new binary downloader with default cache directory
    pub fn new() -> Result<Self> {
        let binaries_dir = Self::default_binaries_dir()?;
        Self::with_binaries_dir(binaries_dir)
    }

    /// Create a downloader with custom binaries directory
    pub fn with_binaries_dir(binaries_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&binaries_dir)?;

        let client = reqwest::Client::builder()
            .user_agent("llm-runtime/0.1.0")
            // Cap connection setup and per-read inactivity so a stalled peer
            // can't hang the data transfer forever. A blanket request timeout
            // is deliberately avoided — large engine downloads are legitimately
            // long-running; read_timeout only fires on no-progress.
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()?;

        Ok(Self {
            binaries_dir,
            client,
        })
    }

    /// Get the default binaries directory
    /// Returns `~/.llm-runtime/binaries/`
    fn default_binaries_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| RuntimeError::internal("Could not determine home directory"))?;

        Ok(home.join(".llm-runtime").join("binaries"))
    }

    /// Download a binary from GitHub releases (no progress reporting).
    /// Thin wrapper around [`Self::download_with_progress`] for callers
    /// that don't need byte-level progress (tests, idempotent re-installs).
    pub async fn download(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
    ) -> Result<BinaryInfo> {
        self.download_with_progress(engine, version, platform, arch, backend, |_, _| {})
            .await
    }

    /// Download a binary from GitHub releases with a per-chunk progress
    /// callback. The callback is invoked synchronously on every chunk
    /// read with `(bytes_received_so_far, total_bytes)`. `total_bytes`
    /// is `None` when the upstream omits Content-Length.
    ///
    /// # Arguments
    /// * `engine` - Engine type (Llamacpp or Mistralrs)
    /// * `version` - Version tag (e.g., "v0.7.0", use "latest" for latest release)
    /// * `platform` - Platform (e.g., "linux", "macos", "windows")
    /// * `arch` - Architecture (e.g., "x86_64", "aarch64")
    /// * `backend` - Backend (e.g., "cpu", "cuda", "metal")
    /// * `progress` - Progress callback (received_bytes, total_bytes)
    pub async fn download_with_progress<F>(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
        progress: F,
    ) -> Result<BinaryInfo>
    where
        F: Fn(u64, Option<u64>) + Send + Sync,
    {
        // Determine repository and binary name (shared naming helpers, so
        // the download URL and asset-readiness detection never drift).
        let repo = engine_repo(engine);
        let binary_name = engine_binary_name(engine, platform);
        let archive_name = archive_name(engine, platform, arch, backend);

        // Resolve version if "latest"
        let resolved_version = if version == "latest" {
            self.get_latest_version(repo).await?
        } else {
            version.to_string()
        };

        tracing::info!(
            "Downloading {} {} for {}-{}-{}",
            match engine {
                EngineType::Llamacpp => "llama-server",
                EngineType::Mistralrs => "mistralrs-server",
            },
            resolved_version,
            platform,
            arch,
            backend
        );

        // Check if already cached
        let cache_dir = self.binaries_dir
            .join(match engine {
                EngineType::Llamacpp => "llamacpp",
                EngineType::Mistralrs => "mistralrs",
            })
            .join(&resolved_version)
            .join(format!("{}-{}-{}", platform, arch, backend));

        let binary_path = cache_dir.join(binary_name);

        if binary_path.exists() {
            tracing::info!("Binary already cached: {}", binary_path.display());
            let metadata = std::fs::metadata(&binary_path)?;

            // Ensure executable on Unix
            #[cfg(unix)]
            super::binary::ensure_executable(&binary_path)?;

            return Ok(BinaryInfo {
                engine,
                version: resolved_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
                backend: backend.to_string(),
                path: binary_path,
                size_bytes: metadata.len(),
            });
        }

        // Construct GitHub release URL (host overridable in debug builds
        // via LLM_RUNTIME_RELEASE_MIRROR for integration tests).
        let download_url = format!(
            "{}/{}/releases/download/{}/{}",
            release_base_url(), repo, resolved_version, archive_name
        );

        tracing::info!("Downloading from: {}", download_url);

        // Create temporary download directory
        let temp_dir = self.binaries_dir.join(".tmp");
        std::fs::create_dir_all(&temp_dir)?;
        let temp_archive = temp_dir.join(&archive_name);

        // Download archive. A miss here is the automated-release race:
        // the tag can exist before CI finishes building + uploading the
        // per-platform binary, so a fetch that 404s means "build pending",
        // not "no such release". Surface that explicitly instead of a bare
        // HTTP error.
        self.download_file(&download_url, &temp_archive, Some(&progress))
            .await
            .map_err(|e| {
                RuntimeError::BinaryNotFound(format!(
                    "engine binary not published for {resolved_version} \
                     {platform}/{arch}/{backend} ({archive_name}): {e}. If the \
                     release was just created, its CI build may still be in \
                     progress — retry later."
                ))
            })?;

        // Best-effort cosign-keyless artifact fetch. We pull the
        // sibling `.sig` when published and log the outcome, but the
        // install proceeds either way — the operator-facing
        // `allow_unsigned_downloads` gate has been removed (downloads
        // are always permitted; cryptographic verification will be
        // re-introduced once the fork CI signs releases). Operators
        // that need stricter handling pre-stage the binary
        // out-of-band.
        let sig_url = format!("{}.sig", download_url);
        let sig_path = temp_dir.join(format!("{}.sig", archive_name));
        // Sig fetch doesn't report progress — it's a tiny artifact and
        // the surrounding download has already left a 100% progress
        // frame in the SSE replay buffer.
        match self.download_file(&sig_url, &sig_path, None).await {
            Ok(()) => {
                tracing::info!(
                    "cosign sibling .sig downloaded for {} (verification not \
                     yet wired — install proceeds unconditionally)",
                    archive_name
                );
            }
            Err(e) => {
                tracing::warn!(
                    "cosign sibling .sig not available for {} ({e}); install \
                     proceeds unverified (TOFU) until the fork CI publishes \
                     signed releases",
                    archive_name
                );
            }
        }

        // Extract binary from archive
        std::fs::create_dir_all(&cache_dir)?;

        if platform == "windows" {
            self.extract_zip(&temp_archive, &cache_dir, binary_name)?;
        } else {
            self.extract_tar_gz(&temp_archive, &cache_dir, binary_name)?;
        }

        // Clean up temporary archive
        std::fs::remove_file(&temp_archive)?;

        // Ensure executable on Unix
        #[cfg(unix)]
        super::binary::ensure_executable(&binary_path)?;

        let metadata = std::fs::metadata(&binary_path)?;

        tracing::info!("Binary downloaded: {}", binary_path.display());

        Ok(BinaryInfo {
            engine,
            version: resolved_version,
            platform: platform.to_string(),
            arch: arch.to_string(),
            backend: backend.to_string(),
            path: binary_path,
            size_bytes: metadata.len(),
        })
    }

    /// Get the latest release version from GitHub
    async fn get_latest_version(&self, repo: &str) -> Result<String> {
        let url = format!("{}/repos/{}/releases/latest", api_base_url(), repo);

        let response = self.client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to get latest release: HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response.json().await?;
        let tag_name = json["tag_name"]
            .as_str()
            .ok_or_else(|| RuntimeError::network("Could not parse latest release tag"))?;

        Ok(tag_name.to_string())
    }

    /// List an engine's upstream releases (newest first, as GitHub returns
    /// them), each reduced to a [`ReleaseInfo`]. Mirror-aware via
    /// [`api_base_url`] (so the integration suite can point it at the mock
    /// release server — same override the download path uses).
    pub async fn list_releases(&self, engine: EngineType) -> Result<Vec<ReleaseInfo>> {
        let url = format!("{}/repos/{}/releases", api_base_url(), engine_repo(engine));

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to list releases: HTTP {}",
                response.status()
            )));
        }

        let releases: Vec<serde_json::Value> = response.json().await?;

        Ok(releases
            .iter()
            .filter_map(|r| {
                let version = r["tag_name"].as_str()?.to_string();
                // GitHub returns `assets[].size` as an integer (bytes).
                // We thread it through to the UI so the download row
                // can show "12.3 MB" before the user clicks Download.
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

    /// Download a file with progress bar.
    ///
    /// Closes 08-llm-local-runtime F-06 (Medium): enforces a 2 GiB
    /// hard cap on downloaded bytes (single engine binary ≤ ~300 MB
    /// in practice; leaves headroom for fat CUDA builds). The
    /// surrounding `client` already has its `timeout()` set in
    /// BinaryDownloader::new; this method adds the size cap as the
    /// remaining missing defense. Without it, an attacker-controlled
    /// upstream (e.g. a hijacked GitHub mirror) could redirect to a
    /// /dev/zero stream and fill the host disk.
    ///
    /// Note: this function does NOT cryptographically verify the
    /// downloaded binary. The right shape is a cosign-keyless verify
    /// (matches the `sigstore` crate already pulled by code_sandbox)
    /// against a `.sig` artifact published alongside each engine
    /// binary. That requires the fork release pipeline to actually
    /// sign (Actions OIDC + cosign sign-blob) — until that ships,
    /// this download path is TOFU. Operators reading the SBOM should
    /// confirm the upstream GitHub Releases page hashes match.
    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: Option<&(dyn Fn(u64, Option<u64>) + Send + Sync)>,
    ) -> Result<()> {
        const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

        // Get file size from HEAD request
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

        // Pre-check the Content-Length when present; fail fast before
        // streaming a single byte.
        if total_size > MAX_DOWNLOAD_BYTES {
            return Err(RuntimeError::network(format!(
                "Refusing to download {} bytes (cap {} bytes / 2 GiB)",
                total_size, MAX_DOWNLOAD_BYTES
            )));
        }

        // No terminal progress bar in the server context — download
        // progress is surfaced to the UI via SSE elsewhere.
        tracing::debug!(
            "Downloading {} ({} bytes)",
            dest.file_name().unwrap().to_string_lossy(),
            total_size
        );

        // Download file
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
        // Initial 0% frame so subscribers see the bar render at start.
        if let Some(cb) = progress {
            cb(0, total_for_cb);
        }

        while let Some(chunk) = response.chunk().await? {
            received = received.saturating_add(chunk.len() as u64);
            if received > MAX_DOWNLOAD_BYTES {
                // Drop the partial download.
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

    /// Extract binary and all shared libraries from tar.gz archive
    fn extract_tar_gz(&self, archive: &Path, dest_dir: &Path, binary_name: &str) -> Result<()> {
        let tar_gz = File::open(archive)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);

        // Enable preservation of permissions, ownership, and symlinks
        archive.set_preserve_permissions(true);
        archive.set_preserve_mtime(true);
        archive.set_unpack_xattrs(true);

        let mut binary_found = false;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_type = entry.header().entry_type();

            // Skip directories
            if entry_type.is_dir() {
                continue;
            }

            // Owned filename (no directory prefix). We FLATTEN every entry
            // into dest_dir, so any archive subdir structure is dropped.
            // Owning it ends the immutable borrow of `entry` before we
            // later need `&mut entry` for `unpack`.
            let file_name = entry
                .path()
                .ok()
                .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                .unwrap_or_default();
            if file_name.is_empty() {
                continue;
            }

            // Hardlinks are still rejected: harder to validate safely and
            // not needed for the SONAME chains we care about.
            if entry_type.is_hard_link() {
                tracing::warn!("Skipping hardlink entry in archive: {}", file_name);
                continue;
            }

            let is_library = file_name.ends_with(".so")
                || file_name.contains(".so.")
                || file_name.ends_with(".dylib")
                || file_name.ends_with(".dll");

            // Symlinks: dynamically-linked engine releases ship SONAME
            // symlinks (`libfoo.so.0 -> libfoo.so.0.1.2`) that the loader
            // NEEDs at runtime — dropping them breaks the engine. We
            // RECREATE a symlink only when it's a library name AND its
            // target is a single, same-directory filename. Anything with
            // an absolute path, `..`, or multiple components is an escape
            // attempt and is rejected — preserving the F-05 path-traversal
            // guard (a `lib_evil.so -> /etc/passwd` link is never created).
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

            // Extract the binary to the root of dest_dir.
            if file_name == binary_name {
                let dest_path = dest_dir.join(binary_name);
                entry.unpack(&dest_path)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            // Extract shared libraries (.so / .dylib / .dll real files).
            if is_library {
                let dest_path = dest_dir.join(&file_name);
                entry.unpack(&dest_path)?;
                tracing::debug!("Extracted library: {}", dest_path.display());
            }
        }

        if !binary_found {
            return Err(RuntimeError::internal(format!(
                "Binary '{}' not found in archive",
                binary_name
            )));
        }

        Ok(())
    }

    /// Extract binary and all DLLs from zip archive
    fn extract_zip(&self, archive: &Path, dest_dir: &Path, binary_name: &str) -> Result<()> {
        let file = File::open(archive)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut binary_found = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name();

            // Skip directories
            if name.ends_with('/') || name.ends_with('\\') {
                continue;
            }

            // Get just the filename without path
            let file_name = std::path::Path::new(name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Extract binary
            if file_name == binary_name {
                let dest_path = dest_dir.join(binary_name);
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            // Extract DLLs (Windows shared libraries)
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
                binary_name
            )));
        }

        Ok(())
    }

    /// Get cached binary if exists
    pub fn get_cached_binary(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
    ) -> Option<PathBuf> {
        let engine_dir = match engine {
            EngineType::Llamacpp => "llamacpp",
            EngineType::Mistralrs => "mistralrs",
        };

        let binary_name = match engine {
            EngineType::Llamacpp => {
                if platform == "windows" { "llama-server.exe" } else { "llama-server" }
            },
            EngineType::Mistralrs => {
                if platform == "windows" { "mistralrs-server.exe" } else { "mistralrs-server" }
            },
        };

        let cache_path = self.binaries_dir
            .join(engine_dir)
            .join(version)
            .join(format!("{}-{}-{}", platform, arch, backend))
            .join(binary_name);

        if cache_path.exists() {
            Some(cache_path)
        } else {
            None
        }
    }

    /// List all cached binaries
    pub fn list_binaries(&self) -> Result<Vec<BinaryInfo>> {
        let mut binaries = Vec::new();

        if !self.binaries_dir.exists() {
            return Ok(binaries);
        }

        // Iterate through engine directories
        for engine_entry in std::fs::read_dir(&self.binaries_dir)? {
            let engine_entry = engine_entry?;
            let engine_dir = engine_entry.path();

            if !engine_dir.is_dir() || engine_dir.file_name().unwrap() == ".tmp" {
                continue;
            }

            let engine = match engine_dir.file_name().unwrap().to_str() {
                Some("llamacpp") => EngineType::Llamacpp,
                Some("mistralrs") => EngineType::Mistralrs,
                _ => continue,
            };

            // Iterate through version directories
            for version_entry in std::fs::read_dir(&engine_dir)? {
                let version_entry = version_entry?;
                let version_dir = version_entry.path();

                if !version_dir.is_dir() {
                    continue;
                }

                let version = version_dir.file_name().unwrap().to_string_lossy().to_string();

                // Iterate through platform-arch-backend directories
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

                    // Find binary file
                    let binary_name = match engine {
                        EngineType::Llamacpp => {
                            if platform == "windows" { "llama-server.exe" } else { "llama-server" }
                        },
                        EngineType::Mistralrs => {
                            if platform == "windows" { "mistralrs-server.exe" } else { "mistralrs-server" }
                        },
                    };

                    let binary_path = build_dir.join(binary_name);

                    if binary_path.exists() {
                        let metadata = std::fs::metadata(&binary_path)?;

                        binaries.push(BinaryInfo {
                            engine,
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
        }

        Ok(binaries)
    }

    /// Get the binaries directory path
    pub fn binaries_dir(&self) -> &Path {
        &self.binaries_dir
    }
}

/// Accept a symlink target only when it names a single entry in the SAME
/// directory (e.g. `libfoo.so.0 -> libfoo.so.0.1.2`). Returns the bare
/// target filename when safe; returns `None` for absolute targets, `..`,
/// or any multi-component path — those are escape attempts and the F-05
/// path-traversal guard rejects them (we flatten everything into one dir,
/// so a same-dir symlink is the only shape we can safely honor).
fn safe_same_dir_symlink_target(link: &Path) -> Option<std::ffi::OsString> {
    use std::path::Component;
    let mut comps = link.components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(name)), None) => Some(name.to_os_string()),
        _ => None,
    }
}

/// Create a relative, same-dir symlink at `link_path` pointing to
/// `target`. Unix-only: on other platforms shared-library SONAME symlinks
/// don't apply (Windows ships DLLs as regular files), so this is a no-op.
fn recreate_symlink(target: &std::ffi::OsStr, link_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        // Remove any stale entry so re-extraction is idempotent.
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

impl Default for BinaryDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default binary downloader")
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Base hosts default to the real GitHub endpoints when the (debug-only)
    /// mirror env vars are unset, and — in debug builds — honor the override
    /// while trimming a trailing slash so URL construction never
    /// double-slashes. Kept as ONE test because it mutates process env; no
    /// other test in this crate touches these vars, so serializing the
    /// assertions here avoids a parallel-execution race.
    #[test]
    fn base_urls_default_to_github_and_honor_mirror_override() {
        // SAFETY: edition-2024 marks env mutation unsafe (it's a process
        // global). This is the only test in the crate touching these vars,
        // so there's no concurrent reader to race with.
        unsafe {
            std::env::remove_var("LLM_RUNTIME_RELEASE_MIRROR");
            std::env::remove_var("LLM_RUNTIME_API_MIRROR");
        }
        assert_eq!(release_base_url(), "https://github.com");
        assert_eq!(api_base_url(), "https://api.github.com");

        // The override path only exists in debug builds.
        #[cfg(debug_assertions)]
        unsafe {
            std::env::set_var("LLM_RUNTIME_RELEASE_MIRROR", "http://127.0.0.1:9999/");
            assert_eq!(release_base_url(), "http://127.0.0.1:9999");
            // Empty is ignored — falls back to the default.
            std::env::set_var("LLM_RUNTIME_RELEASE_MIRROR", "");
            assert_eq!(release_base_url(), "https://github.com");
            std::env::remove_var("LLM_RUNTIME_RELEASE_MIRROR");
        }
    }

    #[test]
    fn safe_symlink_target_accepts_same_dir_rejects_escaping() {
        use std::path::Path;
        // Same-dir SONAME targets are accepted.
        assert_eq!(
            safe_same_dir_symlink_target(Path::new("libfoo.so.0.1.2")).as_deref(),
            Some(std::ffi::OsStr::new("libfoo.so.0.1.2"))
        );
        // Absolute, parent-escaping, and multi-component targets are rejected.
        assert!(safe_same_dir_symlink_target(Path::new("/etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("../../etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("sub/libfoo.so")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("..")).is_none());
    }

    /// A dynamically-linked engine release ships SONAME symlinks the loader
    /// needs (`libfoo.so.1 -> libfoo.so.1.2.3`). The extractor must keep
    /// those (recreated as same-dir relative symlinks) while still
    /// rejecting escaping symlinks (the F-05 guard).
    #[cfg(unix)]
    #[test]
    fn extract_tar_gz_keeps_safe_symlinks_and_rejects_escaping() {
        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("engine.tar.gz");

        {
            let f = File::create(&archive_path).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut b = tar::Builder::new(enc);

            let mut reg = |b: &mut tar::Builder<flate2::write::GzEncoder<File>>, name: &str, data: &[u8], mode: u32| {
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_mode(mode);
                h.set_cksum();
                b.append_data(&mut h, name, data).unwrap();
            };
            // Regular binary + a real versioned library.
            reg(&mut b, "llama-server", b"#!/bin/true\n", 0o755);
            reg(&mut b, "libfoo.so.1.2.3", b"ELF-ish-bytes", 0o644);

            let mut link = |b: &mut tar::Builder<flate2::write::GzEncoder<File>>, name: &str, target: &str| {
                let mut h = tar::Header::new_gnu();
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                h.set_mode(0o777);
                b.append_link(&mut h, name, target).unwrap();
            };
            // SAFE same-dir SONAME symlink.
            link(&mut b, "libfoo.so.1", "libfoo.so.1.2.3");
            // ESCAPING symlinks (absolute + parent-traversal) — must be dropped.
            link(&mut b, "evil.so", "/etc/passwd");
            link(&mut b, "escape.so", "../../etc/passwd");

            b.into_inner().unwrap().finish().unwrap();
        }

        let dest = tmp.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let downloader = BinaryDownloader::with_binaries_dir(tmp.path().join("cache")).unwrap();
        downloader
            .extract_tar_gz(&archive_path, &dest, "llama-server")
            .unwrap();

        // Binary + real lib extracted.
        assert!(dest.join("llama-server").exists());
        assert!(dest.join("libfoo.so.1.2.3").exists());

        // Safe SONAME symlink recreated, relative, and resolves to the real file.
        let link_path = dest.join("libfoo.so.1");
        let meta = std::fs::symlink_metadata(&link_path).unwrap();
        assert!(meta.file_type().is_symlink(), "libfoo.so.1 must be a symlink");
        assert_eq!(
            std::fs::read_link(&link_path).unwrap(),
            std::path::PathBuf::from("libfoo.so.1.2.3")
        );
        assert!(std::fs::canonicalize(&link_path).unwrap().ends_with("libfoo.so.1.2.3"));

        // Escaping symlinks were rejected (never created).
        assert!(dest.join("evil.so").symlink_metadata().is_err(), "absolute-target symlink must be rejected");
        assert!(dest.join("escape.so").symlink_metadata().is_err(), "parent-traversal symlink must be rejected");
    }
}
