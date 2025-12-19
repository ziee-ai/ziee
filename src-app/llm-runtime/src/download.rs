//! Model download system for HuggingFace Hub
//!
//! Downloads GGUF and Safetensor models from HuggingFace with:
//! - Progress bars
//! - Resume support for interrupted downloads
//! - SHA256 checksum verification

use crate::error::{Result, RuntimeError};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// HuggingFace model downloader
pub struct ModelDownloader {
    models_dir: PathBuf,
    client: reqwest::Client,
}

/// Information about a downloaded model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Repository ID (e.g., "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF")
    pub repo_id: String,

    /// Downloaded file name
    pub filename: String,

    /// Local path to the model file
    pub path: PathBuf,

    /// File size in bytes
    pub size_bytes: u64,

    /// SHA256 checksum (if verified)
    pub sha256: Option<String>,
}

impl ModelDownloader {
    /// Create a new model downloader with default models directory
    pub fn new() -> Result<Self> {
        let models_dir = Self::default_models_dir()?;
        Self::with_models_dir(models_dir)
    }

    /// Create a downloader with custom models directory
    pub fn with_models_dir(models_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&models_dir)?;

        let client = reqwest::Client::builder()
            .user_agent("llm-runtime/0.1.0")
            .build()?;

        Ok(Self {
            models_dir,
            client,
        })
    }

    /// Get the default models directory
    /// Returns `~/.llm-runtime/models/`
    fn default_models_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| RuntimeError::internal("Could not determine home directory"))?;

        Ok(home.join(".llm-runtime").join("models"))
    }

    /// Download a model file from HuggingFace
    ///
    /// # Arguments
    /// * `repo_id` - Repository ID (e.g., "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF")
    /// * `filename` - File name to download (e.g., "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf")
    ///
    /// # Returns
    /// Path to the downloaded model file
    pub async fn download(&self, repo_id: &str, filename: &str) -> Result<ModelInfo> {
        // Construct HuggingFace URL
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, filename
        );

        tracing::info!("Downloading from: {}", url);

        // Create local file path
        let repo_name = repo_id.replace('/', "_");
        let local_dir = self.models_dir.join(&repo_name);
        std::fs::create_dir_all(&local_dir)?;

        let local_path = local_dir.join(filename);

        // Check if file already exists and get existing size
        let existing_size = if local_path.exists() {
            std::fs::metadata(&local_path)?.len()
        } else {
            0
        };

        // Get file size from HEAD request
        let head_response = self.client.head(&url).send().await?;

        if !head_response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to access model: HTTP {}",
                head_response.status()
            )));
        }

        let total_size = head_response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| RuntimeError::network("Could not determine file size"))?;

        // Check if file is already fully downloaded
        if existing_size == total_size {
            tracing::info!("File already downloaded: {}", local_path.display());
            return Ok(ModelInfo {
                repo_id: repo_id.to_string(),
                filename: filename.to_string(),
                path: local_path.clone(),
                size_bytes: total_size,
                sha256: None,
            });
        }

        // Setup progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", filename));

        if existing_size > 0 {
            pb.set_position(existing_size);
            tracing::info!("Resuming download from {} bytes", existing_size);
        }

        // Open file for writing (append mode to support resume)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&local_path)?;

        // Make GET request with Range header for resume support
        let mut request = self.client.get(&url);

        if existing_size > 0 {
            request = request.header(
                reqwest::header::RANGE,
                format!("bytes={}-", existing_size),
            );
        }

        let mut response = request.send().await?;

        if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(RuntimeError::network(format!(
                "Failed to download: HTTP {}",
                response.status()
            )));
        }

        // Download file in chunks
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message(format!("Downloaded {}", filename));

        tracing::info!("Download complete: {}", local_path.display());

        Ok(ModelInfo {
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            path: local_path,
            size_bytes: total_size,
            sha256: None,
        })
    }

    /// Verify SHA256 checksum of a downloaded model
    pub fn verify_checksum(&self, model: &mut ModelInfo, expected_sha256: &str) -> Result<bool> {
        tracing::info!("Verifying checksum for: {}", model.path.display());

        let pb = ProgressBar::new(model.size_bytes);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}")
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );
        pb.set_message("Calculating SHA256");

        let mut file = File::open(&model.path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
            pb.inc(bytes_read as u64);
        }

        let result = hasher.finalize();
        let actual_sha256 = format!("{:x}", result);

        pb.finish_with_message("Checksum calculated");

        model.sha256 = Some(actual_sha256.clone());

        let matches = actual_sha256.eq_ignore_ascii_case(expected_sha256);

        if matches {
            tracing::info!("✓ Checksum verified");
        } else {
            tracing::error!(
                "✗ Checksum mismatch!\n  Expected: {}\n  Actual:   {}",
                expected_sha256,
                actual_sha256
            );
        }

        Ok(matches)
    }

    /// List all downloaded models
    pub fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut models = Vec::new();

        if !self.models_dir.exists() {
            return Ok(models);
        }

        for entry in std::fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            let repo_dir = entry.path();

            if !repo_dir.is_dir() {
                continue;
            }

            let repo_id = entry
                .file_name()
                .to_string_lossy()
                .replace('_', "/");

            // List files in repo directory
            for file_entry in std::fs::read_dir(&repo_dir)? {
                let file_entry = file_entry?;
                let file_path = file_entry.path();

                if file_path.is_file() {
                    let metadata = std::fs::metadata(&file_path)?;
                    let filename = file_entry.file_name().to_string_lossy().to_string();

                    models.push(ModelInfo {
                        repo_id: repo_id.clone(),
                        filename,
                        path: file_path,
                        size_bytes: metadata.len(),
                        sha256: None,
                    });
                }
            }
        }

        Ok(models)
    }

    /// Delete a downloaded model
    pub fn delete_model(&self, repo_id: &str, filename: &str) -> Result<()> {
        let repo_name = repo_id.replace('/', "_");
        let local_path = self.models_dir.join(&repo_name).join(filename);

        if local_path.exists() {
            std::fs::remove_file(&local_path)?;
            tracing::info!("Deleted model: {}", local_path.display());

            // Remove directory if empty
            let repo_dir = self.models_dir.join(&repo_name);
            if repo_dir.read_dir()?.next().is_none() {
                std::fs::remove_dir(&repo_dir)?;
                tracing::info!("Removed empty directory: {}", repo_dir.display());
            }
        }

        Ok(())
    }

    /// Get the models directory path
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default model downloader")
    }
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_model_info() {
        let info = ModelInfo {
            repo_id: "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF".to_string(),
            filename: "tiny.gguf".to_string(),
            path: PathBuf::from("/tmp/tiny.gguf"),
            size_bytes: 637000000,
            sha256: None,
        };

        assert_eq!(info.repo_id, "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF");
        assert_eq!(format_bytes(info.size_bytes), "607.49 MB");
    }
}
