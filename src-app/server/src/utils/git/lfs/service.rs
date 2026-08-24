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

/// How long a blob transfer may go with NO bytes arriving before it is killed.
///
/// 60s. The job is to distinguish "silent" from "slow", and silence is the
/// signal: TCP keeps a healthy connection delivering *something* well inside a
/// minute, while a stalled or hostile peer delivers nothing at all. A tighter
/// value (say 10s) would start killing real transfers across a congested link
/// or a server-side seek; a looser one buys the user nothing, because a
/// connection silent for a minute is not coming back inside this download.
/// It is also a 30× improvement on the old bound for the pure-hang case, which
/// took the full 30 minutes to notice.
const LFS_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Absolute backstop on a single blob transfer.
///
/// 6 hours — deliberately NOT the binding constraint on a legitimate pull. At
/// 5.68 GB (the default model's Q4_K_M blob) this implies a floor of ~0.26 MB/s
/// (~2.1 Mbps), i.e. it only fires on a connection slower than early broadband.
/// The old 30-minute cap implied 3.16 MB/s (~25.2 Mbps) on the same file, which
/// is a hard ceiling a user cannot retry their way past — and, because there is
/// no resume, it discarded the whole transfer after ~28 minutes of healthy
/// progress. `lfs_absolute_backstop_is_not_the_binding_constraint` pins this.
const LFS_ABSOLUTE_BACKSTOP: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 6);

/// Absolute budget for the small batch-API metadata POST.
const LFS_METADATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Connect budget, shared by both clients.
const LFS_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Slack allowed over an object's declared size before the stream is aborted.
///
/// The LFS batch API tells us exactly how many bytes an object has, so a server
/// that keeps sending past that is malfunctioning or hostile and there is no
/// reason to keep writing it to disk. Without this, lengthening the absolute
/// timeout would have WIDENED 07-llm-model F-07: the streaming loop had no byte
/// cap at all, so "how much disk can a malicious LFS server consume" was bounded
/// only by the clock — 30 minutes before, 6 hours after. With the cap it is
/// bounded by the object's own size regardless of either timeout, which is
/// strictly stronger than what shipped. 1 MiB of slack keeps a server that pads
/// a final chunk from failing a legitimate download; the checksum still has the
/// final say on correctness.
const LFS_SIZE_SLACK_BYTES: u64 = 1024 * 1024;

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
            .next_back()
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
        let mut parsed = Url::parse(url)?;
        // Host-aware default username, matching the git-clone path
        // (GitService::auth_username_for): "x-access-token" for GitHub, "oauth2"
        // otherwise. NOTE: basic_auth repositories are not fully supported on the
        // LFS path — the configured basic_auth username is not threaded here, so a
        // host-default username is used with the password. The two built-in repos
        // (HF api_key, GitHub bearer_token) are token-based and unaffected (token
        // hosts ignore the username), so this only matters for the uncommon case
        // of a custom basic_auth repo whose model files are LFS-tracked.
        let username = if access_token.is_some() {
            crate::utils::git::GitService::auth_username_for(url, None)
        } else {
            String::new()
        };
        parsed
            .set_username(&username)
            .map_err(|_| LfsError::InvalidFormat("Could not set username"))?;
        parsed
            .set_password(access_token)
            .map_err(|_| LfsError::InvalidFormat("Could not set password"))?;
        Ok(parsed)
    }

    /// Ceiling on bytes accepted for an object of `declared` size.
    ///
    /// Saturating so a malicious `size` near `u64::MAX` cannot wrap the ceiling
    /// down to a small number and make every honest chunk look oversized.
    fn max_object_bytes(declared: u64) -> u64 {
        declared.saturating_add(LFS_SIZE_SLACK_BYTES)
    }

    /// Would accepting `chunk_len` more bytes push the transfer past `max_bytes`?
    fn exceeds_declared_size(downloaded: u64, chunk_len: u64, max_bytes: u64) -> bool {
        downloaded.saturating_add(chunk_len) > max_bytes
    }

    /// Client for the git-lfs **batch API** call — a small JSON POST.
    ///
    /// Kept on its own tight absolute budget. The blob client below deliberately
    /// tolerates a transfer that runs for hours; applying that to a metadata
    /// request would let a hostile endpoint pin a task for the same period for
    /// the sake of a few hundred bytes. Two clients rather than one client with
    /// a per-request override, so neither call can inherit the other's budget by
    /// accident — the override form makes the tight bound a property of one call
    /// site that a later edit can silently drop.
    fn metadata_client(absolute: std::time::Duration) -> Result<Client, LfsError> {
        Ok(Client::builder()
            .timeout(absolute)
            .connect_timeout(LFS_CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?)
    }

    /// Client for the **blob** GET — potentially many GB.
    ///
    /// `read_timeout` bounds each read of the body and resets on every
    /// successful read (reqwest's documented semantics), so a connection that is
    /// alive but slow survives while one that goes silent is cut off promptly.
    /// The absolute timeout stays as a backstop so total time is still bounded.
    fn blob_client(
        stall: std::time::Duration,
        absolute: std::time::Duration,
    ) -> Result<Client, LfsError> {
        Ok(Client::builder()
            .read_timeout(stall)
            .timeout(absolute)
            .connect_timeout(LFS_CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?)
    }

    /// Create the temp file an LFS object streams into, inside `staging_dir`.
    ///
    /// Split out of `download_file` purely so this — the code that failed on the
    /// owner's machine — is reachable from a test without standing up an LFS
    /// server. `download_file` itself needs a live HTTP endpoint; this does not.
    async fn create_staging_file(
        staging_dir: &Path,
        oid: &str,
        randomizer_bytes: Option<usize>,
    ) -> Result<NamedTempFile, LfsError> {
        debug!("creating temp file in staging dir {:?}", staging_dir);

        const TEMP_SUFFIX: &str = ".lfstmp";
        // Staged in `staging_dir`, NEVER in the CWD — see `download_file`'s doc
        // comment for what CWD staging cost on macOS.
        let tmp_path = staging_dir.join(format!("{oid}{TEMP_SUFFIX}"));

        if randomizer_bytes.is_none() && tmp_path.exists() {
            debug!("temp file exists. Deleting");
            fs::remove_file(&tmp_path).await?;
        }

        tempfile::Builder::new()
            .prefix(oid)
            .suffix(TEMP_SUFFIX)
            .rand_bytes(randomizer_bytes.unwrap_or_default())
            .tempfile_in(staging_dir)
            .map_err(|e| LfsError::TempFile(e.to_string()))
    }

    /// `staging_dir` is where the multi-GB object is written while it streams.
    ///
    /// It is a PARAMETER rather than a constant because the previous `"./"`
    /// staged in the process's current working directory, and a process may not
    /// assume anything about its CWD. A macOS `.app` launched from Finder
    /// inherits `CWD = /`, which since 10.15 is the read-only Signed System
    /// Volume, so every LFS download died instantly with EROFS — reported by the
    /// owner as `Read-only file system (os error 30) at path
    /// "/./<oid>.lfstmp"`. (The `/./` is the fingerprint of the bug: `tempfile`
    /// absolutizes a relative base against CWD, and `absolute()` does not
    /// normalize `.`.)
    ///
    /// Callers pass the object's CACHE directory, which makes the final
    /// `rename` same-directory — hence atomic and same-filesystem — by
    /// construction.
    async fn download_file(
        meta_data: &LfsMetadata,
        repo_remote_url: &str,
        access_token: Option<&str>,
        randomizer_bytes: Option<usize>,
        progress_tx: Option<&mpsc::UnboundedSender<LfsProgress>>,
        base_progress: u64,
        total_size_all_files: u64,
        staging_dir: &Path,
    ) -> Result<NamedTempFile, LfsError> {
        const MEDIA_TYPE: &str = "application/vnd.git-lfs+json";
        // SECURITY: bound both HTTP calls with explicit timeouts, a redirect cap
        // and — for the blob — a hard byte cap. Closes 07-llm-model F-07
        // (Medium). See DEC-19: the bound is a STALL timeout plus a size cap,
        // not the former 30-minute absolute cap, which could not tell a slow
        // download from a malicious one and so failed healthy multi-GB pulls.
        let metadata_client = Self::metadata_client(LFS_METADATA_TIMEOUT)?;
        let client = Self::blob_client(LFS_STALL_TIMEOUT, LFS_ABSOLUTE_BACKSTOP)?;

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
        // The METADATA client — tightly bounded. Not `client`, which tolerates a
        // multi-hour blob transfer.
        let response = metadata_client
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
                Err(LfsError::ResponseNotOkay(status.as_u16()))
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

        let temp_file =
            Self::create_staging_file(staging_dir, &meta_data.oid, randomizer_bytes).await?;

        debug!("created tempfile: {:?}", &temp_file);

        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = 0u64;
        // Don't overwrite total_size parameter - it contains the sum of all files
        // meta_data.size is only the size of the current file

        // SECURITY (F-07): the object's size is known from the batch API, so a
        // server that keeps sending past it is malfunctioning or hostile. Abort
        // rather than writing unbounded bytes to disk — this, not the clock, is
        // what bounds disk consumption now that the absolute timeout is hours.
        let max_bytes = Self::max_object_bytes(meta_data.size);

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            if Self::exceeds_declared_size(downloaded_bytes, chunk.len() as u64, max_bytes) {
                error!(
                    "LFS object {} sent more than its declared size ({} bytes); aborting",
                    &meta_data.oid, meta_data.size
                );
                return Err(LfsError::InvalidResponse(format!(
                    "LFS server sent more data than the object's declared size ({} bytes)",
                    meta_data.size
                )));
            }
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

            // Stage IN the destination directory (created just above). The
            // object is multi-GB, so this also keeps the bytes on the volume
            // they will live on rather than copying them across afterwards.
            let temp_file = Self::download_file(
                metadata,
                &repo_url,
                access_token,
                randomizer_bytes,
                progress_tx,
                base_progress,
                total_size_all_files,
                &cache_dir,
            )
            .await?;

            if cache_file.exists() {
                info!(
                    "cache file {:?} is already written from other process",
                    &cache_file
                );
            } else {
                // `rename` fails with EXDEV (Cross-device link) when the temp
                // file's filesystem differs from the cache dir's.
                //
                // KEPT DELIBERATELY, though it is now unreachable (DEC-17).
                // The temp file is staged IN `cache_dir`, so both paths are in
                // one directory and therefore one filesystem — EXDEV cannot
                // occur today. It is retained because `staging_dir` is a
                // PARAMETER: a future caller that stages elsewhere makes this
                // reachable again, and the cost of keeping it is one comparison
                // on an error path that a multi-GB download would otherwise
                // fail outright.
                //
                // (The comment this replaces claimed "tempfile picks the OS
                // default /tmp". That was never true — it picked `./`, the
                // process CWD, which is the bug this round fixes.)
                if let Err(e) =
                    fs::rename(&temp_file.path(), cache_file.as_path()).await
                {
                    if e.raw_os_error() == Some(libc::EXDEV) {
                        info!(
                            "rename across filesystems failed (EXDEV); falling back to copy+remove for {:?} -> {:?}",
                            temp_file.path(),
                            cache_file.as_path(),
                        );
                        fs::copy(&temp_file.path(), cache_file.as_path())
                            .await
                            .map_err(|e| {
                                error!(
                                    "Could not copy {:?} to {:?}: {:?}",
                                    temp_file.path(),
                                    cache_file.as_path(),
                                    &e
                                );
                                LfsError::Io(e)
                            })?;
                        // Best-effort cleanup. NOT "the OS reaps /tmp anyway":
                        // the temp file lives in the LFS cache dir, which
                        // nothing reaps. `NamedTempFile`'s own Drop is the
                        // backstop if this fails.
                        let _ = fs::remove_file(temp_file.path()).await;
                    } else {
                        error!(
                            "Could not rename {:?} to {:?}: {:?}",
                            temp_file.path(),
                            cache_file.as_path(),
                            &e
                        );
                        return Err(LfsError::Io(e));
                    }
                }
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
            .map_err(LfsError::Io)?;

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
        if let Some(ref token) = cancellation_token
            && token.is_cancelled().await {
                return Err(LfsError::Cancelled);
            }

        // First scan which of the requested files are LFS pointers
        let mut lfs_files = Vec::new();
        let mut total_size = 0u64;

        for file_path in file_paths {
            // Check for cancellation during scan
            if let Some(ref token) = cancellation_token
                && token.is_cancelled().await {
                    return Err(LfsError::Cancelled);
                }

            let full_path = repo_path.join(file_path);

            // Use the existing is_lfs_pointer_file function to check if file is an LFS pointer
            if let Ok(is_lfs) = is_lfs_pointer_file(&full_path).await
                && is_lfs {
                    // Read the file content to get metadata
                    if let Ok(content) = fs::read_to_string(&full_path).await
                        && let Some((_oid, size)) = parse_lfs_pointer_content(&content) {
                            lfs_files.push(LfsPointer {
                                size,
                                path: PathBuf::from(file_path),
                            });
                            total_size += size;
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
            if let Some(ref token) = cancellation_token
                && token.is_cancelled().await {
                    return Err(LfsError::Cancelled);
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
        if let Some(ref token) = cancellation_token
            && token.is_cancelled().await {
                return Err(LfsError::Cancelled);
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

    // --- FB-3: an LFS object must never be staged in the process CWD ---------
    //
    // The owner's macOS `.app` inherited `CWD = /` (the read-only Signed System
    // Volume) and every download died instantly with
    // `Read-only file system (os error 30) at path "/./<oid>.lfstmp"`.
    //
    // Both tests below assert the SAME invariant from opposite sides, and both
    // go red if the staging base reverts to `"./"`.

    const TEST_OID: &str = "03b74727a860a56338e042c4420bb3f04b2fec5734175f4cb9fa853daf52b7e8";

    #[tokio::test]
    async fn staging_file_is_created_in_the_directory_it_was_given() {
        let staging = tempfile::tempdir().expect("staging dir");
        let file = LfsService::create_staging_file(staging.path(), TEST_OID, None)
            .await
            .expect("staging file should be creatable in a writable dir");

        // The PARENT is the assertion. "the download succeeded" would pass on
        // the broken code whenever the CWD happened to be writable, which is
        // exactly why this bug survived to a release.
        let parent = file.path().parent().expect("temp file has a parent");
        assert_eq!(
            parent.canonicalize().expect("parent canonicalize"),
            staging.path().canonicalize().expect("staging canonicalize"),
            "LFS objects must stage in the directory passed by the caller, not the process CWD",
        );
    }

    #[tokio::test]
    async fn staging_leaves_no_temp_file_in_the_process_cwd() {
        // The other side of the invariant, and the closest safe reproduction of
        // the owner's failure. A literal read-only-CWD test would have to mutate
        // the PROCESS-GLOBAL current directory inside a test binary that runs
        // ~1500 tests in parallel threads, which risks breaking unrelated tests
        // that resolve relative paths — a worse outcome than the coverage gained
        // (and a child-process harness to isolate it would be far more apparatus
        // than this ten-line fix warrants). Asserting the CWD stays CLEAN pins
        // the same property without touching it: under the old `"./"` base the
        // temp file landed here.
        let cwd = std::env::current_dir().expect("cwd");
        let before = count_lfstmp(&cwd);

        let staging = tempfile::tempdir().expect("staging dir");
        let file = LfsService::create_staging_file(staging.path(), TEST_OID, None)
            .await
            .expect("staging file");

        assert!(file.path().exists(), "the staged file should exist");
        assert_eq!(
            count_lfstmp(&cwd),
            before,
            "staging must not create a .lfstmp in the process CWD — on a read-only CWD that is an outright EROFS failure",
        );
    }

    // --- FB-4 / DEC-19: the transfer bound is a STALL, not a wall clock -------
    //
    // The old `.timeout(30min)` was an ABSOLUTE cap on the whole request body,
    // so at 5.68 GB it imposed a 3.16 MB/s (~25.2 Mbps) floor that no amount of
    // retrying could get past — and with no resume, it discarded ~28 minutes of
    // healthy progress. These tests pin the replacement's BEHAVIOUR on a real
    // socket, with millisecond budgets so nothing sleeps for real.

    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    /// Serve one HTTP response whose body is dribbled `chunks` × 1 byte at
    /// `gap`, then (if `then_go_silent`) hold the connection open forever
    /// without sending the rest.
    async fn dribble_server(
        chunks: usize,
        gap: Duration,
        then_go_silent: bool,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let declared = if then_go_silent { chunks + 1000 } else { chunks };
        let handle = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\n\r\n"
                );
                if sock.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                for _ in 0..chunks {
                    if sock.write_all(b"x").await.is_err() {
                        return;
                    }
                    let _ = sock.flush().await;
                    tokio::time::sleep(gap).await;
                }
                if then_go_silent {
                    // Never send the remaining bytes, never close.
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        });
        (format!("http://{addr}/"), handle)
    }

    #[tokio::test]
    async fn a_slow_but_progressing_transfer_survives_an_absolute_cap_that_would_kill_it() {
        // 20 bytes at 20ms = ~400ms of steady progress.
        let (url, server) = dribble_server(20, Duration::from_millis(20), false).await;

        // NEW shape: generous absolute backstop + a stall bound that keeps
        // resetting because bytes keep arriving.
        let ok = Self_blob(Duration::from_millis(150), Duration::from_secs(30))
            .get(&url)
            .send()
            .await
            .expect("request")
            .bytes()
            .await;
        assert!(ok.is_ok(), "a steadily-progressing transfer must not be killed: {ok:?}");
        assert_eq!(ok.unwrap().len(), 20);

        // POSITIVE CONTROL — the OLD shape (absolute cap only, no read timeout)
        // kills that same healthy stream. This is the 30-minute cap in
        // miniature, and it is why the owner could not download 5.68 GB.
        let (url2, server2) = dribble_server(20, Duration::from_millis(20), false).await;
        let old = Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
            .expect("client")
            .get(&url2)
            .send()
            .await
            .expect("request")
            .bytes()
            .await;
        assert!(
            old.is_err(),
            "control failed: the old absolute-cap-only config should kill a healthy slow stream",
        );

        server.abort();
        server2.abort();
    }

    #[tokio::test]
    async fn a_transfer_that_goes_silent_is_cut_off_promptly() {
        // SECURITY (F-07): a peer that stops sending must not pin the task.
        // Sends 3 bytes, promises 1003, then goes quiet forever.
        let (url, server) = dribble_server(3, Duration::from_millis(5), true).await;

        let started = std::time::Instant::now();
        let result = Self_blob(Duration::from_millis(150), Duration::from_secs(30))
            .get(&url)
            .send()
            .await
            .expect("request")
            .bytes()
            .await;

        assert!(result.is_err(), "a silent peer must be cut off, not awaited");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the stall bound must fire promptly, not wait for the absolute backstop",
        );
        server.abort();
    }

    #[tokio::test]
    async fn the_metadata_call_keeps_its_own_tight_budget() {
        // The regression a naive fix causes: pointing the small batch-API POST
        // at the blob client would let a hostile endpoint hold a task for the
        // blob's multi-hour budget.
        let (url, server) = dribble_server(1, Duration::from_millis(5), true).await;

        let started = std::time::Instant::now();
        let meta = LfsService::metadata_client(Duration::from_millis(200))
            .expect("metadata client")
            .post(&url)
            .send()
            .await;
        let elapsed_or_body = match meta {
            Ok(resp) => resp.bytes().await.err().map(|_| ()),
            Err(_) => Some(()),
        };
        assert!(
            elapsed_or_body.is_some(),
            "the metadata call must be bounded by its own absolute budget",
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the metadata budget must not have been widened to the blob's",
        );
        server.abort();
    }

    #[test]
    fn lfs_absolute_backstop_is_not_the_binding_constraint() {
        // The arithmetic that made the old cap a hard ceiling. The default
        // model's blob is 5.68 GB; the backstop must not imply a throughput
        // floor a home connection cannot meet.
        const BLOB_BYTES: f64 = 5.68 * 1000.0 * 1000.0 * 1000.0;
        let floor_mbps =
            (BLOB_BYTES * 8.0) / LFS_ABSOLUTE_BACKSTOP.as_secs() as f64 / 1_000_000.0;
        assert!(
            floor_mbps < 5.0,
            "backstop implies a {floor_mbps:.1} Mbps floor — the old 30-min cap implied 25.2 Mbps, which is what broke the owner's download",
        );
        // And the stall bound must stay tight enough to be a real security bound.
        assert!(
            LFS_STALL_TIMEOUT <= Duration::from_secs(120),
            "a stall bound this loose stops being a bound",
        );
    }

    #[test]
    fn an_object_may_not_exceed_its_declared_size() {
        // SECURITY (F-07): with the absolute timeout now measured in hours, THIS
        // is what bounds how much disk a hostile LFS server can consume. The
        // streaming loop previously had no byte cap at all.
        let max = LfsService::max_object_bytes(1_000);
        assert!(!LfsService::exceeds_declared_size(0, 1_000, max), "the exact size must be accepted");
        assert!(
            !LfsService::exceeds_declared_size(1_000, LFS_SIZE_SLACK_BYTES, max),
            "a padded final chunk within the slack must not fail a real download",
        );
        assert!(
            LfsService::exceeds_declared_size(1_000, LFS_SIZE_SLACK_BYTES + 1, max),
            "a server sending past the declared size + slack must be cut off",
        );
        // A declared size near u64::MAX must not wrap the ceiling to a tiny
        // number — that would reject every honest chunk instead of accepting it.
        assert_eq!(LfsService::max_object_bytes(u64::MAX), u64::MAX);
        assert!(!LfsService::exceeds_declared_size(u64::MAX - 1, 1, u64::MAX));
    }

    /// Local alias so the tests read as "the blob client".
    #[allow(non_snake_case)]
    fn Self_blob(stall: Duration, absolute: Duration) -> Client {
        LfsService::blob_client(stall, absolute).expect("blob client")
    }

    fn count_lfstmp(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| {
                        e.file_name().to_string_lossy().ends_with(".lfstmp")
                    })
                    .count()
            })
            .unwrap_or(0)
    }
}
