use super::metadata::{is_lfs_pointer_file, parse_lfs_pointer_content};
use super::{FilePullMode, LfsError, LfsMetadata, LfsPhase, LfsPointer, LfsProgress};
use crate::utils::cancellation::CancellationToken;
use futures_util::stream::StreamExt;
use http::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use url::Url;

#[derive(Deserialize, Debug)]
struct ApiResult {
    objects: Vec<Object>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Object {
    oid: String,
    size: usize,
    actions: Option<Action>,
    authenticated: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Action {
    download: Download,
}

#[derive(Deserialize, Serialize, Debug)]
struct Download {
    href: String,
    #[serde(default)]
    header: HashMap<String, String>,
}

impl Object {
    fn from_metadata(input: &LfsMetadata) -> Self {
        Object {
            oid: input.oid.clone(),
            size: input.size as usize,
            actions: None,
            authenticated: None,
        }
    }
}

pub struct LfsService {
    // Field removed as it was never accessed - methods use static get_cache_dir instead
}

impl LfsService {
    pub fn new(_cache_dir: PathBuf) -> Self {
        Self {}
    }

    /// Find the git repository root folder of the given file
    async fn get_repo_root<P: AsRef<Path>>(file_or_path: P) -> Result<PathBuf, LfsError> {
        info!(
            "Searching git repo root from path {}",
            file_or_path.as_ref().display()
        );

        let repo_dir = fs::canonicalize(file_or_path.as_ref()).await.map_err(|e| {
            LfsError::DirectoryTraversalError(format!(
                "Problem getting the absolute path of {}: {}",
                file_or_path.as_ref().display(),
                e
            ))
        })?;

        let components: Vec<_> = repo_dir.components().collect();
        for i in (0..components.len()).rev() {
            let path = components
                .iter()
                .take(i)
                .fold(PathBuf::new(), |a, b| a.join(b));
            if path.join(".git").exists() {
                return Ok(path);
            }
        }

        Err(LfsError::DirectoryTraversalError(format!(
            "Could not find .git in any parent path of the given path ({})",
            file_or_path.as_ref().display()
        )))
    }

    /// Get remote URL from git config
    async fn get_remote_url<P: AsRef<Path>>(repo_path: P) -> Result<String, LfsError> {
        let config_file = Self::get_real_repo_root(repo_path.as_ref())
            .await?
            .join(".git")
            .join("config");

        Self::get_remote_url_from_file(config_file).await
    }

    async fn get_remote_url_from_file<P: AsRef<Path>>(git_file: P) -> Result<String, LfsError> {
        let file_buffer = fs::read_to_string(git_file).await?;
        let remote_url = file_buffer
            .lines()
            .find(|&line| line.contains("url"))
            .ok_or(LfsError::InvalidFormat(
                ".git/config contains no remote url",
            ))?
            .split('=')
            .last()
            .ok_or(LfsError::InvalidFormat(".git/config url line malformed"))?
            .trim();
        Ok(remote_url.to_owned())
    }

    async fn get_real_repo_root<P: AsRef<Path>>(repo_path: P) -> Result<PathBuf, LfsError> {
        let git_path = repo_path.as_ref().join(".git");
        let real_git_path = if repo_path.as_ref().join(".git").is_file() {
            // worktree case
            let worktree_file_contents = fs::read_to_string(git_path).await?;
            let worktree_path = worktree_file_contents
                .split(':')
                .find(|c| c.contains(".git"))
                .ok_or_else(|| {
                    LfsError::DirectoryTraversalError(
                        "Could not resolve original repo .git/config file from worktree .git file"
                            .to_string(),
                    )
                })?
                .trim();
            Self::get_repo_root(worktree_path).await.map_err(|_| {
                LfsError::DirectoryTraversalError(
                    "Found worktree, but couldn't resolve root-repo".to_string(),
                )
            })?
        } else if git_path.is_dir() {
            // git main copy
            git_path
                .parent()
                .ok_or_else(|| {
                    LfsError::DirectoryTraversalError("Git path has no parent".to_string())
                })?
                .to_owned()
        } else {
            // no .git in repo_root - bad
            return Err(LfsError::DirectoryTraversalError(
                "Could not find .git file or folder in directory structure".to_owned(),
            ));
        };

        Ok(real_git_path)
    }

    fn remote_url_ssh_to_https(repo_url: String) -> Result<String, LfsError> {
        let input_url = Url::parse(&repo_url)?;
        if input_url.scheme() == "https" {
            return Ok(repo_url);
        } else if input_url.scheme() != "ssh" {
            return Err(LfsError::InvalidFormat("Url is neither https nor ssh"));
        }
        let host = input_url
            .host_str()
            .ok_or(LfsError::InvalidFormat("Url had no valid host"))?;
        let path = input_url.path();
        Ok(format!("https://{}{}", host, path))
    }

    async fn get_cache_dir<P: AsRef<Path>>(
        repo_root: P,
        metadata: &LfsMetadata,
    ) -> Result<PathBuf, LfsError> {
        let oid_1 = &metadata.oid[0..2];
        let oid_2 = &metadata.oid[2..4];

        Ok(Self::get_real_repo_root(repo_root)
            .await?
            .join(".git")
            .join("lfs")
            .join("objects")
            .join(oid_1)
            .join(oid_2))
    }

    fn url_with_auth(url: &str, access_token: Option<&str>) -> Result<Url, LfsError> {
        let mut url = Url::parse(url)?;
        let username = if access_token.is_some() { "oauth2" } else { "" };
        url.set_username(username)
            .map_err(|_| LfsError::InvalidFormat("Could not set username"))?;
        url.set_password(access_token)
            .map_err(|_| LfsError::InvalidFormat("Could not set password"))?;
        Ok(url)
    }

    async fn download_file(
        meta_data: &LfsMetadata,
        repo_remote_url: &str,
        access_token: Option<&str>,
        randomizer_bytes: Option<usize>,
        progress_tx: Option<&mpsc::UnboundedSender<LfsProgress>>,
        base_progress: u64,
        total_size_all_files: u64,
    ) -> Result<NamedTempFile, LfsError> {
        const MEDIA_TYPE: &str = "application/vnd.git-lfs+json";
        // SECURITY: bound the HTTP client with explicit timeouts and a
        // redirect cap. The previous `Client::builder().build()` used
        // reqwest defaults (no overall timeout, no per-request budget,
        // up to 10 redirects with no per-host filter). Closes
        // 07-llm-model F-07 (Medium).
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60 * 30)) // 30-min absolute cap (LFS blobs can be GB)
            .connect_timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;

        if meta_data.hash != Some(super::metadata::Hash::SHA256) {
            return Err(LfsError::InvalidFormat("Only SHA256 hash is supported"));
        }

        // Implement git-lfs batch API: https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md
        let request = json!({
            "operation": "download",
            "transfers": [ "basic" ],
            "ref": {"name" : "refs/heads/main" },
            "objects": vec![Object::from_metadata(meta_data)],
            "hash_algo": "sha256"
        });

        // if repo_remote_url not ends with .git, append it
        let repo_remote_url = if repo_remote_url.ends_with(".git") {
            repo_remote_url.to_string()
        } else {
            format!("{}.git", repo_remote_url)
        };

        let request_url = repo_remote_url.to_owned() + "/info/lfs/objects/batch";
        let request_url = Self::url_with_auth(&request_url, access_token)?;
        let response = client
            .post(request_url.clone())
            .header("Accept", MEDIA_TYPE)
            .header("Content-Type", MEDIA_TYPE)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!(
                "Failed to request git lfs actions with status code {} and body {}",
                status, body
            );

            return if status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED {
                Err(LfsError::AccessDenied)
            } else if status == StatusCode::NOT_FOUND && body.contains("Cannot POST") {
                // Likely a repository that doesn't support Git LFS batch API (e.g., some Hugging Face repos)
                Err(LfsError::InvalidResponse(format!(
                    "Repository does not support Git LFS batch API. This may be a Hugging Face repository without LFS enabled, or the files may not be LFS files. Status: {}",
                    status
                )))
            } else {
                Err(LfsError::ResponseNotOkay(format!("{}", status)))
            };
        }

        // Get response text for debugging before parsing
        let response_text = response.text().await?;
        debug!("LFS batch API response: {}", response_text);

        let parsed_result: ApiResult = serde_json::from_str(&response_text).map_err(|e| {
            LfsError::InvalidResponse(format!("Failed to parse LFS response: {}", e))
        })?;

        // Download the file
        let object = parsed_result
            .objects
            .first()
            .ok_or(LfsError::RemoteFileNotFound(
                "Empty object list response from LFS server",
            ))?;

        let action = object.actions.as_ref().ok_or(LfsError::RemoteFileNotFound(
            "No action received from LFS server",
        ))?;

        // SECURITY: validate the action.download.href against the SSRF
        // policy before fetching. The href is server-controlled by the
        // LFS server we just talked to; a malicious or compromised repo
        // could return an action pointing at AWS IMDS / RFC 1918 / a
        // file:// path, and we'd happily fetch it WITH the auth token
        // attached. Closes 07-llm-model F-01 (Critical) LFS-side.
        if let Err(e) = crate::utils::url_validator::validate_outbound_url(
            &action.download.href,
            &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
        ) {
            return Err(LfsError::InvalidFormat(Box::leak(
                format!("LFS download URL rejected by SSRF policy: {}", e).into_boxed_str(),
            )));
        }
        let url = Self::url_with_auth(&action.download.href, access_token)?;
        let headers: http::HeaderMap = (&action.download.header).try_into()?;
        let download_request_builder = client.get(url).headers(headers);
        let response = download_request_builder.send().await?;
        let download_status = response.status();

        if !download_status.is_success() {
            let message = format!(
                "Download failed: {} - body {}",
                download_status,
                response.text().await.unwrap_or_default()
            );
            return Err(LfsError::InvalidResponse(message));
        }

        debug!("creating temp file in current dir");

        const TEMP_SUFFIX: &str = ".lfstmp";
        const TEMP_FOLDER: &str = "./";
        let tmp_path = PathBuf::from(TEMP_FOLDER).join(format!("{}{TEMP_SUFFIX}", &meta_data.oid));

        if randomizer_bytes.is_none() && tmp_path.exists() {
            debug!("temp file exists. Deleting");
            fs::remove_file(&tmp_path).await?;
        }

        let temp_file = tempfile::Builder::new()
            .prefix(&meta_data.oid)
            .suffix(TEMP_SUFFIX)
            .rand_bytes(randomizer_bytes.unwrap_or_default())
            .tempfile_in(TEMP_FOLDER)
            .map_err(|e| LfsError::TempFile(e.to_string()))?;

        debug!("created tempfile: {:?}", &temp_file);

        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = 0u64;
        // Don't overwrite total_size parameter - it contains the sum of all files
        // meta_data.size is only the size of the current file

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            temp_file.as_file().write_all(&chunk).map_err(|e| {
                error!("Could not write tempfile");
                LfsError::Io(e)
            })?;
            hasher.update(&chunk);

            // Update progress
            downloaded_bytes += chunk.len() as u64;
            if let Some(tx) = progress_tx {
                let current_total_progress = base_progress + downloaded_bytes;
                let _ = tx.send(LfsProgress {
                    phase: LfsPhase::Downloading,
                    current: current_total_progress,
                    total: total_size_all_files,
                    message: format!(
                        "Downloading... {:.1}%",
                        (current_total_progress as f64 / total_size_all_files as f64) * 100.0
                    ),
                });
            }
        }

        temp_file.as_file().flush().map_err(|e| {
            error!("Could not flush tempfile");
            LfsError::Io(e)
        })?;

        debug!("checking hash");

        let result = hasher.finalize();
        let hex_data = hex::decode(object.oid.as_bytes())?;

        if result[..] == hex_data {
            Ok(temp_file)
        } else {
            Err(LfsError::ChecksumMismatch)
        }
    }

    async fn get_file_cached<P: AsRef<Path>>(
        repo_root: P,
        metadata: &LfsMetadata,
        access_token: Option<&str>,
        randomizer_bytes: Option<usize>,
        progress_tx: Option<&mpsc::UnboundedSender<LfsProgress>>,
        base_progress: u64,
        total_size_all_files: u64,
    ) -> Result<(PathBuf, FilePullMode), LfsError> {
        let cache_dir = Self::get_cache_dir(&repo_root, metadata).await?;
        debug!("cache dir {:?}", &cache_dir);
        let cache_file = cache_dir.join(&metadata.oid);
        debug!("cache file {:?}", &cache_file);
        let repo_url = Self::remote_url_ssh_to_https(Self::get_remote_url(&repo_root).await?)?;

        if cache_file.is_file() {
            Ok((cache_file, FilePullMode::UsedLocalCache))
        } else {
            fs::create_dir_all(&cache_dir).await.map_err(|_| {
                LfsError::DirectoryTraversalError(
                    "Could not create lfs cache directory".to_string(),
                )
            })?;

            let temp_file = Self::download_file(
                metadata,
                &repo_url,
                access_token,
                randomizer_bytes,
                progress_tx,
                base_progress,
                total_size_all_files,
            )
            .await?;

            if cache_file.exists() {
                info!(
                    "cache file {:?} is already written from other process",
                    &cache_file
                );
            } else {
                fs::rename(&temp_file.path(), cache_file.as_path())
                    .await
                    .map_err(|e| {
                        error!(
                            "Could not rename {:?} to {:?}: {:?}",
                            temp_file.path(),
                            cache_file.as_path(),
                            &e
                        );
                        LfsError::Io(e)
                    })?;
            }

            Ok((cache_file, FilePullMode::DownloadedFromRemote))
        }
    }

    /// Pull a single LFS file
    pub async fn pull_file<P: AsRef<Path>>(
        lfs_file: P,
        access_token: Option<&str>,
        randomizer_bytes: Option<usize>,
        progress_tx: Option<&mpsc::UnboundedSender<LfsProgress>>,
        base_progress: Option<u64>,
        total_size_all_files: Option<u64>,
    ) -> Result<FilePullMode, LfsError> {
        info!("Pulling file {}", lfs_file.as_ref().display());

        if !is_lfs_pointer_file(&lfs_file).await? {
            info!(
                "File ({}) not an lfs-node file - pulled already.",
                lfs_file
                    .as_ref()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            return Ok(FilePullMode::WasAlreadyPresent);
        }

        debug!("parsing metadata");
        let metadata = LfsMetadata::parse_from_file(&lfs_file).await?;
        debug!("Downloading file");

        let repo_root = Self::get_repo_root(&lfs_file).await?;

        let (file_name_cached, origin) = Self::get_file_cached(
            &repo_root,
            &metadata,
            access_token,
            randomizer_bytes,
            progress_tx,
            base_progress.unwrap_or(0),
            total_size_all_files.unwrap_or(metadata.size),
        )
        .await?;

        info!(
            "Found file (Origin: {:?}), linking to {}",
            origin,
            lfs_file.as_ref().display()
        );

        fs::remove_file(&lfs_file).await?;
        fs::hard_link(&file_name_cached, lfs_file)
            .await
            .map_err(|e| LfsError::Io(e))?;

        Ok(origin)
    }

    /// Pull multiple LFS files with progress and cancellation support
    /// This replaces the `pull_lfs_files_with_cancellation` function from git_service.rs
    pub async fn pull_lfs_files_with_cancellation(
        &self,
        repo_path: &Path,
        file_paths: &[String],
        auth_token: Option<&str>,
        progress_tx: mpsc::UnboundedSender<LfsProgress>,
        cancellation_token: Option<CancellationToken>,
    ) -> Result<(), LfsError> {
        info!("Starting LFS file pull for {} files", file_paths.len());

        // Send initial progress
        let _ = progress_tx.send(LfsProgress {
            phase: LfsPhase::Scanning,
            current: 0,
            total: 0,
            message: "Starting LFS file scan...".to_string(),
        });

        if file_paths.is_empty() {
            let _ = progress_tx.send(LfsProgress {
                phase: LfsPhase::Complete,
                current: 100,
                total: 100,
                message: "No LFS files to download".to_string(),
            });
            return Ok(());
        }

        // Check for cancellation before starting
        if let Some(ref token) = cancellation_token {
            if token.is_cancelled().await {
                return Err(LfsError::Cancelled);
            }
        }

        // First scan which of the requested files are LFS pointers
        let mut lfs_files = Vec::new();
        let mut total_size = 0u64;

        for file_path in file_paths {
            // Check for cancellation during scan
            if let Some(ref token) = cancellation_token {
                if token.is_cancelled().await {
                    return Err(LfsError::Cancelled);
                }
            }

            let full_path = repo_path.join(file_path);

            // Use the existing is_lfs_pointer_file function to check if file is an LFS pointer
            if let Ok(is_lfs) = is_lfs_pointer_file(&full_path).await {
                if is_lfs {
                    // Read the file content to get metadata
                    if let Ok(content) = fs::read_to_string(&full_path).await {
                        if let Some((_oid, size)) = parse_lfs_pointer_content(&content) {
                            lfs_files.push(LfsPointer {
                                size,
                                path: PathBuf::from(file_path),
                            });
                            total_size += size;
                        }
                    }
                }
            }
        }

        info!(
            "Found {} LFS files with total size {} bytes",
            lfs_files.len(),
            total_size
        );

        if lfs_files.is_empty() {
            let _ = progress_tx.send(LfsProgress {
                phase: LfsPhase::Complete,
                current: 0,
                total: 0,
                message: "No LFS files found to download".to_string(),
            });
            return Ok(());
        }

        // Download files
        let mut downloaded_size = 0u64;
        let total_files = lfs_files.len();

        for (index, lfs_pointer) in lfs_files.iter().enumerate() {
            // Check for cancellation before each file
            if let Some(ref token) = cancellation_token {
                if token.is_cancelled().await {
                    return Err(LfsError::Cancelled);
                }
            }

            let file_name = lfs_pointer
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Send progress update for starting this file
            let _ = progress_tx.send(LfsProgress {
                phase: LfsPhase::Downloading,
                current: downloaded_size,
                total: total_size,
                message: format!(
                    "Downloading {} ({} of {})",
                    file_name,
                    index + 1,
                    total_files
                ),
            });

            // Download the file
            let full_file_path = repo_path.join(&lfs_pointer.path);
            match Self::pull_file(
                &full_file_path,
                auth_token,
                None,
                Some(&progress_tx),
                Some(downloaded_size),
                Some(total_size),
            )
            .await
            {
                Ok(_) => {
                    downloaded_size += lfs_pointer.size;

                    let _ = progress_tx.send(LfsProgress {
                        phase: LfsPhase::Downloading,
                        current: downloaded_size,
                        total: total_size,
                        message: format!(
                            "Completed {} ({} of {})",
                            file_name,
                            index + 1,
                            total_files
                        ),
                    });
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to download LFS file {}: {}",
                        lfs_pointer.path.display(),
                        e
                    );
                    let _ = progress_tx.send(LfsProgress {
                        phase: LfsPhase::Error,
                        current: 0,
                        total: 100,
                        message: error_msg,
                    });
                    return Err(e);
                }
            }
        }

        // Check for cancellation one final time
        if let Some(ref token) = cancellation_token {
            if token.is_cancelled().await {
                return Err(LfsError::Cancelled);
            }
        }

        // All files downloaded successfully
        let _ = progress_tx.send(LfsProgress {
            phase: LfsPhase::Complete,
            current: total_size,
            total: total_size,
            message: format!("Successfully downloaded all {} LFS files", total_files),
        });

        info!(
            "LFS download completed: {} files, {} total bytes",
            total_files, total_size
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_to_https_transform() {
        let repo_remote = "ssh://git@github.com/user/repo.git";
        let repo_remote_https = "https://github.com/user/repo.git";
        let result = LfsService::remote_url_ssh_to_https(repo_remote.to_string())
            .expect("Could not parse url");
        assert_eq!(result, repo_remote_https);
    }

    #[test]
    fn test_https_identity() {
        let repo_remote_https = "https://github.com/user/repo.git";
        let result = LfsService::remote_url_ssh_to_https(repo_remote_https.to_string())
            .expect("Could not parse url");
        assert_eq!(result, repo_remote_https);
    }
}
