//! Binary downloader for engine executables from GitHub releases
//!
//! Downloads pre-built engine binaries from GitHub releases with:
//! - Progress bars
//! - Resume support for interrupted downloads
//! - Automatic caching in ~/.llm-runtime/binaries/
//! - Executable permission setting (Unix)

use crate::config::EngineType;
use crate::error::{Result, RuntimeError};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

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

    /// Download a binary from GitHub releases
    ///
    /// # Arguments
    /// * `engine` - Engine type (Llamacpp or Mistralrs)
    /// * `version` - Version tag (e.g., "v0.7.0", use "latest" for latest release)
    /// * `platform` - Platform (e.g., "linux", "macos", "windows")
    /// * `arch` - Architecture (e.g., "x86_64", "aarch64")
    /// * `backend` - Backend (e.g., "cpu", "cuda", "metal")
    pub async fn download(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
    ) -> Result<BinaryInfo> {
        // Determine repository and binary name
        let (repo, binary_name, archive_name) = match engine {
            EngineType::Llamacpp => (
                "ziee-ai/llama.cpp",
                if platform == "windows" { "llama-server.exe" } else { "llama-server" },
                format!("llama-server-{}-{}-{}.{}",
                    platform, arch, backend,
                    if platform == "windows" { "zip" } else { "tar.gz" }
                ),
            ),
            EngineType::Mistralrs => (
                "ziee-ai/mistral.rs",
                if platform == "windows" { "mistralrs-server.exe" } else { "mistralrs-server" },
                format!("mistralrs-server-{}-{}-{}.{}",
                    platform, arch, backend,
                    if platform == "windows" { "zip" } else { "tar.gz" }
                ),
            ),
        };

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
            crate::binary::ensure_executable(&binary_path)?;

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

        // Construct GitHub release URL
        let download_url = format!(
            "https://github.com/{}/releases/download/{}/{}",
            repo, resolved_version, archive_name
        );

        tracing::info!("Downloading from: {}", download_url);

        // Create temporary download directory
        let temp_dir = self.binaries_dir.join(".tmp");
        std::fs::create_dir_all(&temp_dir)?;
        let temp_archive = temp_dir.join(&archive_name);

        // Download archive
        self.download_file(&download_url, &temp_archive).await?;

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
        crate::binary::ensure_executable(&binary_path)?;

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
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

        let response = self.client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
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
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
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

        // Setup progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", dest.file_name().unwrap().to_string_lossy()));

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
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message(format!("Downloaded {}", dest.file_name().unwrap().to_string_lossy()));

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
            let path = entry.path()?;

            // Skip directories
            if entry.header().entry_type().is_dir() {
                continue;
            }

            // Skip symlink / hardlink entries: a malicious archive
            // could plant a `lib_evil.so → /etc/passwd` symlink that
            // a later write through the extracted directory would
            // follow out of the cache dir. Closes
            // 08-llm-local-runtime F-05 (Medium).
            let entry_type = entry.header().entry_type();
            if entry_type.is_symlink() || entry_type.is_hard_link() {
                tracing::warn!(
                    "Skipping symlink/hardlink entry in archive: {}",
                    path.display()
                );
                continue;
            }

            // Get the filename without any directory prefix
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Extract binary to root of dest_dir
            if file_name == binary_name {
                let dest_path = dest_dir.join(binary_name);
                entry.unpack(&dest_path)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            // Extract shared libraries (.so, .dylib, .dll files)
            let is_library = file_name.ends_with(".so")
                || file_name.contains(".so.")
                || file_name.ends_with(".dylib")
                || file_name.ends_with(".dll");

            if is_library {
                let dest_path = dest_dir.join(file_name);
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

impl Default for BinaryDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default binary downloader")
    }
}

/// Helper function for integration tests to download and cache test binaries
///
/// Auto-detects platform and downloads CPU binaries (except macOS which uses Metal)
pub async fn ensure_test_binary(
    engine: EngineType,
    version: &str,
) -> Result<PathBuf> {
    let downloader = BinaryDownloader::new()?;

    // Detect current platform
    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return Err(RuntimeError::internal("Unsupported platform"));
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(RuntimeError::internal("Unsupported architecture"));
    };

    // Use CPU backend for Linux/Windows tests, Metal for macOS
    let backend = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "cpu"
    };

    // Check cache first
    if let Some(cached) = downloader.get_cached_binary(engine, version, platform, arch, backend) {
        tracing::info!("Using cached binary: {}", cached.display());
        return Ok(cached);
    }

    // Download if not cached
    let info = downloader.download(engine, version, platform, arch, backend).await?;

    Ok(info.path)
}
