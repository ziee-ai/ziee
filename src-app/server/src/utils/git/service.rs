use super::lfs::{LfsError, LfsPhase, LfsProgress, LfsService};
use crate::utils::cancellation::CancellationToken;
use git2::{Cred, FetchOptions, RemoteCallbacks, build::RepoBuilder};
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct GitProgress {
    pub phase: GitPhase,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum GitPhase {
    Connecting,
    Receiving,
    Resolving,
    CheckingOut,
    Complete,
    Error,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Operation was cancelled")]
    Cancelled,
    #[error("Invalid repository URL: {0}")]
    InvalidUrl(String),
    /// The remote rejected the request with 401/403 (auth failed or
    /// insufficient permissions). A typed variant so callers can classify the
    /// failure without string-matching the Display message.
    #[error("Access denied (401/403): {0}")]
    AccessDenied(String),
    /// The remote responded with a non-success HTTP status during an LFS
    /// transfer. Carries the status code so callers classify by value.
    #[error("Remote HTTP error {status}: {message}")]
    HttpStatus { status: u16, message: String },
}

/// Per-cache-key serialization to prevent two concurrent clones of the
/// same repo from racing each other through `open_existing_repo` (each
/// would think the other's partial checkout is corrupt + retry from
/// scratch). Closes 09-llm-repository F-09 (Medium). The map is
/// process-global because cache directories are process-global.
static CLONE_LOCKS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn lock_for(cache_key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut map = CLONE_LOCKS.lock().unwrap_or_else(|p| p.into_inner());
    map.entry(cache_key.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

pub struct GitService {
    cache_dir: std::path::PathBuf,
    lfs_service: LfsService,
}

impl GitService {
    /// Construct with explicit cache directories. Caller passes the
    /// paths resolved from `core::config::CachesConfig` (typically via
    /// the global `crate::core::get_caches_config()`).
    pub fn with_cache_dirs(
        cache_dir: std::path::PathBuf,
        lfs_cache_dir: std::path::PathBuf,
    ) -> Self {
        let lfs_service = LfsService::new(lfs_cache_dir);
        Self {
            cache_dir,
            lfs_service,
        }
    }

    /// Construct from the global `CachesConfig`. Convenience for handlers
    /// that don't have the resolved Config in scope — reads from the
    /// `CACHES_CONFIG` static populated at server boot.
    pub fn new() -> Self {
        let caches = crate::core::get_caches_config();
        Self::with_cache_dirs(
            std::path::PathBuf::from(caches.git_cache_dir()),
            std::path::PathBuf::from(caches.lfs_cache_dir()),
        )
    }

    /// Generate a unique cache key based on repository_id, URL, and branch
    fn generate_cache_key(
        repository_id: &Uuid,
        repository_url: &str,
        branch: Option<&str>,
    ) -> String {
        let mut hasher = DefaultHasher::new();
        repository_id.hash(&mut hasher);
        repository_url.hash(&mut hasher);
        branch.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{}-{:x}", repository_id, hash)
    }


    /// Clone a repository with cancellation support (LFS files not included in initial clone)
    pub async fn clone_repository(
        &self,
        repository_url: &str,
        repository_id: &Uuid,
        branch: Option<&str>,
        auth_token: Option<&str>,
        // Explicit username for basic_auth (paired with the password in
        // auth_token). None for token auth (api_key/bearer_token), where a
        // host-appropriate default username is chosen instead.
        auth_username: Option<&str>,
        progress_tx: mpsc::UnboundedSender<GitProgress>,
        cancellation_token: Option<CancellationToken>,
    ) -> Result<std::path::PathBuf, GitError> {
        // SECURITY: validate the repository URL against the SSRF policy
        // BEFORE any DNS / network activity. The URL flows from admin
        // input (llm_repository or hub config); the upstream validate_url
        // in modules/llm_repository/utils.rs already screens this at
        // insert time, but this is the defense-in-depth check at the
        // git-level entry point so any future caller path (e.g., direct
        // git clone calls bypassing the repository module) is also
        // covered. Closes 07-llm-model F-01 (Critical) clone-side.
        if let Err(e) = crate::utils::url_validator::validate_outbound_url(
            repository_url,
            &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
        ) {
            return Err(GitError::InvalidUrl(format!(
                "repository URL rejected by SSRF policy: {}",
                e
            )));
        }

        // Serialize concurrent clones of the same (repo_id, url, branch).
        // Closes 09-llm-repository F-09 (Medium): two callers used to
        // race the cache directory open + partial checkout, each
        // seeing the other's in-progress state as corrupt and
        // retrying from scratch. The async Mutex held for the entire
        // clone is fine because clones are minutes-scale operations
        // anyway; the contention window matches the natural
        // sequential ordering callers would want.
        let cache_key_lock_str =
            Self::generate_cache_key(repository_id, repository_url, branch);
        let clone_lock = lock_for(&cache_key_lock_str);
        let _clone_guard = clone_lock.lock().await;

        // Check for cancellation before starting
        if let Some(ref token) = cancellation_token
            && token.is_cancelled().await {
                return Err(GitError::Cancelled);
            }

        // Generate cache key based on repository_id, URL, and branch
        let cache_key = Self::generate_cache_key(repository_id, repository_url, branch);
        let repo_cache_dir = self.cache_dir.join(cache_key);

        // Check if the cache folder already exists and is a valid git repository
        let is_existing_repo = repo_cache_dir.exists() && repo_cache_dir.join(".git").exists();

        // Ensure cache directory exists
        tokio::fs::create_dir_all(&self.cache_dir).await?;

        let progress_tx_clone = progress_tx.clone();
        let repo_cache_dir_clone = repo_cache_dir.clone();
        let repository_url = repository_url.to_string();
        let auth_token = auth_token.map(|s| s.to_string());
        let auth_username = auth_username.map(|s| s.to_string());
        let branch = branch.map(|s| s.to_string());

        // Create a cancellation flag for the blocking task
        let cancelled_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled_flag_task = cancelled_flag.clone();

        // Spawn a task to monitor cancellation and update the flag
        let cancellation_monitor = if let Some(token) = cancellation_token.clone() {
            let flag = cancelled_flag.clone();
            Some(tokio::spawn(async move {
                while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if token.is_cancelled().await {
                        flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }))
        } else {
            None
        };

        // Run git operations in a blocking task (merged implementation from clone_repository_blocking)
        let result = tokio::task::spawn_blocking(move || {
            if is_existing_repo {
                // Repository exists, pull latest changes
                let _ = progress_tx_clone.send(GitProgress {
                    phase: GitPhase::Connecting,
                    current: 10,
                    total: 100,
                    message: "Opening existing repository".to_string(),
                });

                let repo = match git2::Repository::open(&repo_cache_dir_clone) {
                    Ok(repo) => repo,
                    Err(e) => {
                        let _ = progress_tx_clone.send(GitProgress {
                            phase: GitPhase::Error,
                            current: 0,
                            total: 100,
                            message: format!("Failed to open repository: {}", e),
                        });
                        return Err(GitError::Git(e));
                    }
                };

                // Check for cancellation before pull
                if let Some(ref token) = cancellation_token {
                    let rt = tokio::runtime::Handle::try_current();
                    if let Ok(handle) = rt {
                        let token_clone = token.clone();
                        let cancelled = handle.block_on(async { token_clone.is_cancelled().await });
                        if cancelled {
                            return Err(GitError::Cancelled);
                        }
                    }
                }

                let _ = progress_tx_clone.send(GitProgress {
                    phase: GitPhase::Connecting,
                    current: 30,
                    total: 100,
                    message: format!("Fetching updates from {}", repository_url),
                });

                // Set up callbacks for fetch operation
                let mut callbacks = RemoteCallbacks::new();

                // SECURITY: only return credentials when libgit2 calls
                // the callback for the ORIGINAL repository host. Without
                // this pin, a server-controlled redirect or hostname
                // alias would receive the auth token. Closes
                // 09-llm-repository F-12 (Medium).
                let original_host =
                    reqwest::Url::parse(&repository_url).ok().and_then(|u| {
                        u.host_str().map(|h| h.to_lowercase())
                    });
                callbacks.credentials(move |url, username_from_url, _allowed_types| {
                    // Compare the callback's URL host to the original;
                    // refuse credentials on mismatch.
                    let cb_host = reqwest::Url::parse(url)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_lowercase()));
                    if original_host.is_some() && cb_host != original_host {
                        tracing::warn!(
                            original = ?original_host,
                            callback = ?cb_host,
                            "git credential callback fired for a different host; refusing token"
                        );
                        return Err(git2::Error::from_str(
                            "credentials refused: callback host doesn't match original",
                        ));
                    }
                    if let Some(token) = auth_token.as_deref() {
                        // Pair the secret with a NON-EMPTY username (HuggingFace
                        // rejects an empty one): an explicit basic_auth username
                        // wins; otherwise use a host-appropriate default. For
                        // api_key/bearer_token auth_username is None, so this is
                        // exactly the token path.
                        let user = auth_username
                            .as_deref()
                            .filter(|u| !u.is_empty())
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| {
                                GitService::auth_username_for(url, username_from_url)
                            });
                        Cred::userpass_plaintext(&user, token)
                    } else {
                        Cred::default()
                    }
                });

                // Set up progress callback
                let cancelled_flag_callback = cancelled_flag_task.clone();
                let progress_tx_callback = progress_tx_clone.clone();
                callbacks.transfer_progress(move |progress| {
                    // Check for cancellation using atomic flag
                    if cancelled_flag_callback.load(std::sync::atomic::Ordering::Relaxed) {
                        tracing::info!("Git fetch cancelled by user");
                        return false;
                    }

                    // Use git2's byte progress if available, otherwise estimate from objects
                    let current_bytes = if progress.received_bytes() > 0 {
                        progress.received_bytes() as u64
                    } else {
                        // Fallback: estimate bytes from objects (roughly 10KB per object)
                        progress.received_objects() as u64 * 10240
                    };

                    // Git2 doesn't provide total_bytes, so estimate from objects
                    let total_bytes = if progress.total_objects() > 0 {
                        progress.total_objects() as u64 * 10240
                    } else {
                        100 * 1024 * 1024 // Default 100MB estimate
                    };

                    let _ = progress_tx_callback.send(GitProgress {
                        phase: GitPhase::Receiving,
                        current: current_bytes,
                        total: total_bytes,
                        message: format!(
                            "Receiving objects: {} / {}",
                            progress.received_objects(),
                            progress.total_objects()
                        ),
                    });

                    true
                });

                let mut fetch_options = git2::FetchOptions::new();
                fetch_options.remote_callbacks(callbacks);

                // Get the origin remote and fetch
                let mut remote = match repo.find_remote("origin") {
                    Ok(remote) => remote,
                    Err(e) => {
                        let _ = progress_tx_clone.send(GitProgress {
                            phase: GitPhase::Error,
                            current: 0,
                            total: 100,
                            message: format!("Failed to find origin remote: {}", e),
                        });
                        return Err(GitError::Git(e));
                    }
                };

                // Fetch from remote
                match remote.fetch(&[] as &[&str], Some(&mut fetch_options), None) {
                    Ok(_) => {
                        let _ = progress_tx_clone.send(GitProgress {
                            phase: GitPhase::CheckingOut,
                            current: 90,
                            total: 100,
                            message: "Updating working directory".to_string(),
                        });

                        // Get the target branch or default to main/master
                        let branch_name = branch.as_deref().unwrap_or("main");
                        let remote_branch_name = format!("origin/{}", branch_name);

                        // Try to find the remote branch
                        match repo.find_branch(&remote_branch_name, git2::BranchType::Remote) {
                            Ok(remote_branch) => {
                                let target_commit = remote_branch.get().target().unwrap();

                                // Reset HEAD to the remote branch
                                let target_commit_obj = repo.find_commit(target_commit).unwrap();
                                match repo.reset(
                                    target_commit_obj.as_object(),
                                    git2::ResetType::Hard,
                                    None,
                                ) {
                                    Ok(_) => Ok(()),
                                    Err(e) => {
                                        let _ = progress_tx_clone.send(GitProgress {
                                            phase: GitPhase::Error,
                                            current: 0,
                                            total: 100,
                                            message: format!("Failed to reset to latest: {}", e),
                                        });
                                        Err(GitError::Git(e))
                                    }
                                }
                            }
                            Err(_) => {
                                // Try master if main doesn't exist
                                let master_branch_name = "origin/master";
                                match repo.find_branch(master_branch_name, git2::BranchType::Remote)
                                {
                                    Ok(remote_branch) => {
                                        let target_commit = remote_branch.get().target().unwrap();
                                        let target_commit_obj =
                                            repo.find_commit(target_commit).unwrap();
                                        match repo.reset(
                                            target_commit_obj.as_object(),
                                            git2::ResetType::Hard,
                                            None,
                                        ) {
                                            Ok(_) => Ok(()),
                                            Err(e) => {
                                                let _ = progress_tx_clone.send(GitProgress {
                                                    phase: GitPhase::Error,
                                                    current: 0,
                                                    total: 100,
                                                    message: format!(
                                                        "Failed to reset to latest: {}",
                                                        e
                                                    ),
                                                });
                                                Err(GitError::Git(e))
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = progress_tx_clone.send(GitProgress {
                                            phase: GitPhase::Error,
                                            current: 0,
                                            total: 100,
                                            message: format!("Failed to find remote branch: {}", e),
                                        });
                                        Err(GitError::Git(e))
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if e.code() == git2::ErrorCode::User
                            && let Some(ref token) = cancellation_token {
                                let rt = tokio::runtime::Handle::try_current();
                                if let Ok(handle) = rt {
                                    let token_clone = token.clone();
                                    let cancelled =
                                        handle.block_on(async { token_clone.is_cancelled().await });
                                    if cancelled {
                                        return Err(GitError::Cancelled);
                                    }
                                }
                            }

                        let _ = progress_tx_clone.send(GitProgress {
                            phase: GitPhase::Error,
                            current: 0,
                            total: 100,
                            message: format!("Failed to fetch updates: {}", e),
                        });
                        Err(GitError::Git(e))
                    }
                }
            } else {
                // Repository doesn't exist, perform initial clone
                let mut callbacks = RemoteCallbacks::new();

                // SECURITY: pin credentials to the original repository
                // host — see fetch path above. Closes 09-llm-repository F-12.
                let original_host_clone =
                    reqwest::Url::parse(&repository_url).ok().and_then(|u| {
                        u.host_str().map(|h| h.to_lowercase())
                    });
                callbacks.credentials(move |url, username_from_url, _allowed_types| {
                    let cb_host = reqwest::Url::parse(url)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_lowercase()));
                    if original_host_clone.is_some() && cb_host != original_host_clone {
                        tracing::warn!(
                            original = ?original_host_clone,
                            callback = ?cb_host,
                            "git credential callback fired for a different host; refusing token"
                        );
                        return Err(git2::Error::from_str(
                            "credentials refused: callback host doesn't match original",
                        ));
                    }
                    if let Some(token) = auth_token.as_deref() {
                        // Pair the secret with a NON-EMPTY username: an explicit
                        // basic_auth username wins; otherwise a host-appropriate
                        // default (HuggingFace rejects an empty one). For
                        // api_key/bearer_token auth_username is None, so this is
                        // exactly the token path.
                        let user = auth_username
                            .as_deref()
                            .filter(|u| !u.is_empty())
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| {
                                GitService::auth_username_for(url, username_from_url)
                            });
                        Cred::userpass_plaintext(&user, token)
                    } else {
                        // Try default credentials
                        Cred::default()
                    }
                });

                // Set up progress callback with cancellation check
                let cancelled_flag_callback = cancelled_flag_task.clone();
                let progress_tx_callback = progress_tx_clone.clone();
                // 09-llm-repository F-05 (High): cap total clone bytes.
                // Without this, an attacker who controls the repo URL
                // (or even a benign mis-sized HF repo) can fill the host
                // disk. 10 GB cap matches the largest legitimate model
                // weights (Llama-70B + LFS pointers); operators with
                // genuine larger needs can override via config in a
                // follow-up. The transfer_progress callback returns
                // false to cancel; libgit2 surfaces that as
                // ErrorCode::User.
                const MAX_CLONE_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GiB
                callbacks.transfer_progress(move |progress| {
                    // Check for cancellation using atomic flag
                    if cancelled_flag_callback.load(std::sync::atomic::Ordering::Relaxed) {
                        tracing::info!("Git clone cancelled by user");
                        return false; // Cancel the operation
                    }

                    // Enforce clone size cap. Use received_bytes when
                    // available (libgit2 ≥ 0.99); otherwise estimate at
                    // 10KB/object to fail-closed on object-count
                    // explosion.
                    let bytes_seen = if progress.received_bytes() > 0 {
                        progress.received_bytes() as u64
                    } else {
                        progress.received_objects() as u64 * 10240
                    };
                    if bytes_seen > MAX_CLONE_BYTES {
                        tracing::error!(
                            received = bytes_seen,
                            cap = MAX_CLONE_BYTES,
                            "Git clone exceeded size cap; aborting"
                        );
                        return false;
                    }

                    let phase = if progress.received_objects() == progress.total_objects() {
                        if progress.indexed_deltas() == progress.total_deltas() {
                            GitPhase::CheckingOut
                        } else {
                            GitPhase::Resolving
                        }
                    } else {
                        GitPhase::Receiving
                    };

                    // Use git2's byte progress if available, otherwise estimate from objects
                    let current_bytes = if progress.received_bytes() > 0 {
                        progress.received_bytes() as u64
                    } else {
                        // Fallback: estimate bytes from objects (roughly 10KB per object)
                        progress.received_objects() as u64 * 10240
                    };

                    // Git2 doesn't provide total_bytes, so estimate from objects
                    let total_bytes = if progress.total_objects() > 0 {
                        progress.total_objects() as u64 * 10240
                    } else {
                        100 * 1024 * 1024 // Default 100MB estimate
                    };

                    let message = match phase {
                        GitPhase::Receiving => format!(
                            "Receiving objects: {} / {}",
                            progress.received_objects(),
                            progress.total_objects()
                        ),
                        GitPhase::Resolving => format!(
                            "Resolving deltas: {} / {}",
                            progress.indexed_deltas(),
                            progress.total_deltas()
                        ),
                        GitPhase::CheckingOut => "Checking out files...".to_string(),
                        _ => "Processing...".to_string(),
                    };

                    let _ = progress_tx_callback.send(GitProgress {
                        phase,
                        current: current_bytes,
                        total: total_bytes,
                        message,
                    });

                    true
                });

                // Set up fetch options
                let mut fetch_options = FetchOptions::new();
                fetch_options.remote_callbacks(callbacks);

                // Send connecting message
                let _ = progress_tx_clone.send(GitProgress {
                    phase: GitPhase::Connecting,
                    current: 0,
                    total: 100,
                    message: format!("Connecting to {}", repository_url),
                });

                // Perform the clone using RepoBuilder
                let mut builder = RepoBuilder::new();
                builder.fetch_options(fetch_options);

                // Set branch if specified
                if let Some(branch_name) = branch.as_deref() {
                    builder.branch(branch_name);
                }

                // Check for cancellation before clone
                if let Some(ref token) = cancellation_token {
                    let rt = tokio::runtime::Handle::try_current();
                    if let Ok(handle) = rt {
                        let token_clone = token.clone();
                        let cancelled = handle.block_on(async { token_clone.is_cancelled().await });
                        if cancelled {
                            return Err(GitError::Cancelled);
                        }
                    }
                }

                match builder.clone(&repository_url, &repo_cache_dir_clone) {
                    Ok(_) => {
                        // Don't fetch LFS files during initial clone
                        Ok(())
                    }
                    Err(e) => {
                        // Check if error was due to cancellation
                        if e.code() == git2::ErrorCode::User {
                            // Progress callback returned false, likely due to cancellation
                            if let Some(ref token) = cancellation_token {
                                let rt = tokio::runtime::Handle::try_current();
                                if let Ok(handle) = rt {
                                    let token_clone = token.clone();
                                    let cancelled =
                                        handle.block_on(async { token_clone.is_cancelled().await });
                                    if cancelled {
                                        return Err(GitError::Cancelled);
                                    }
                                }
                            }
                        }

                        // Send error progress before returning
                        let _ = progress_tx_clone.send(GitProgress {
                            phase: GitPhase::Error,
                            current: 0,
                            total: 100,
                            message: format!("Clone failed: {}", e),
                        });
                        Err(GitError::Git(e))
                    }
                }
            }
        })
        .await
        .map_err(|e| GitError::Git(git2::Error::from_str(&e.to_string())))?;

        // Clean up the cancellation monitor
        if let Some(monitor) = cancellation_monitor {
            monitor.abort();
        }

        match result {
            Ok(_) => {
                let message = if is_existing_repo {
                    "Repository updated successfully"
                } else {
                    "Repository cloned successfully"
                };

                let _ = progress_tx.send(GitProgress {
                    phase: GitPhase::Complete,
                    current: 1, // Completion - we don't know exact bytes, so use 1:1 ratio
                    total: 1,
                    message: message.to_string(),
                });
                Ok(repo_cache_dir)
            }
            Err(e) => {
                let message = if is_existing_repo {
                    format!("Update failed: {}", e)
                } else {
                    format!("Clone failed: {}", e)
                };

                let _ = progress_tx.send(GitProgress {
                    phase: GitPhase::Error,
                    current: 0,
                    total: 0,
                    message,
                });
                Err(e)
            }
        }
    }

    /// Build repository URL from repository configuration
    pub fn build_repository_url(base_url: &str, repository_path: &str) -> String {
        // Remove trailing slash from base_url
        let base_url = base_url.trim_end_matches('/');

        match base_url {
            url if url.contains("github.com") => {
                format!("{}/{}.git", base_url, repository_path)
            }
            url if url.contains("huggingface.co") => {
                format!("{}/{}", base_url, repository_path)
            }
            _ => {
                format!("{}/{}.git", base_url, repository_path)
            }
        }
    }

    /// Pick the git HTTP username to pair with a token for a given host.
    ///
    /// Token-based hosts (HuggingFace, GitLab, ...) ignore the username value but
    /// REQUIRE it to be non-empty over HTTPS Basic auth; an empty username makes
    /// HuggingFace reject the clone with "git failed to authenticate". GitHub
    /// conventionally uses "x-access-token". An explicit non-empty username from
    /// the URL always wins. The LFS client (`lfs::service::url_with_auth`) routes
    /// its default username through this same helper so the two paths agree.
    pub fn auth_username_for(url: &str, username_from_url: Option<&str>) -> String {
        if let Some(u) = username_from_url {
            if !u.is_empty() {
                return u.to_string();
            }
        }
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
            .unwrap_or_default();
        // Exact host (or real subdomain) match — `contains` would also classify a
        // spoof host like "github.com.evil.example" as GitHub.
        if host == "github.com" || host.ends_with(".github.com") {
            "x-access-token".to_string()
        } else {
            "oauth2".to_string()
        }
    }

    /// Pull specific LFS files based on file paths with cancellation support
    /// Now uses the native LFS implementation instead of git-lfs binary
    pub async fn pull_lfs_files_with_cancellation(
        &self,
        repo_path: &Path,
        file_paths: &[String],
        auth_token: Option<&str>,
        progress_tx: mpsc::UnboundedSender<GitProgress>,
        cancellation_token: Option<CancellationToken>,
    ) -> Result<(), GitError> {
        // Create a channel to receive LFS progress updates
        let (lfs_progress_tx, mut lfs_progress_rx) = mpsc::unbounded_channel::<LfsProgress>();

        tracing::info!(
            "Pulling LFS files from repository: {} with paths: {:?}",
            repo_path.display(),
            file_paths
        );

        // Spawn a task to convert LFS progress to Git progress
        let git_progress_tx = progress_tx.clone();
        let progress_converter = tokio::spawn(async move {
            while let Some(lfs_progress) = lfs_progress_rx.recv().await {
                let git_progress = GitProgress {
                    phase: match lfs_progress.phase {
                        LfsPhase::Scanning => GitPhase::Connecting,
                        LfsPhase::Downloading => GitPhase::CheckingOut,
                        LfsPhase::Complete => GitPhase::Complete,
                        LfsPhase::Error => GitPhase::Error,
                    },
                    current: lfs_progress.current,
                    total: lfs_progress.total,
                    message: lfs_progress.message,
                };

                if git_progress_tx.send(git_progress).is_err() {
                    break; // Channel closed
                }
            }
        });

        // Use the new LFS service
        let result = self
            .lfs_service
            .pull_lfs_files_with_cancellation(
                repo_path,
                file_paths,
                auth_token,
                lfs_progress_tx,
                cancellation_token,
            )
            .await
            .map_err(|e| match e {
                LfsError::Cancelled => GitError::Cancelled,
                LfsError::Io(io_err) => GitError::Io(io_err),
                LfsError::AccessDenied => GitError::AccessDenied(e.to_string()),
                LfsError::ResponseNotOkay(status) => GitError::HttpStatus {
                    status,
                    message: e.to_string(),
                },
                _ => GitError::Git(git2::Error::from_str(&e.to_string())),
            });

        // Clean up the progress converter task
        progress_converter.abort();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::GitService;

    #[test]
    fn auth_username_defaults_oauth2_for_huggingface() {
        assert_eq!(
            GitService::auth_username_for("https://huggingface.co/meta-llama/Llama-3.1-8B", None),
            "oauth2"
        );
    }

    #[test]
    fn auth_username_x_access_token_for_github() {
        assert_eq!(
            GitService::auth_username_for("https://github.com/owner/repo.git", None),
            "x-access-token"
        );
    }

    #[test]
    fn auth_username_explicit_non_empty_wins() {
        assert_eq!(
            GitService::auth_username_for("https://huggingface.co/x/y", Some("alice")),
            "alice"
        );
    }

    #[test]
    fn auth_username_empty_falls_through_to_host_default() {
        assert_eq!(
            GitService::auth_username_for("https://github.com/x/y", Some("")),
            "x-access-token"
        );
        assert_eq!(
            GitService::auth_username_for("https://huggingface.co/x/y", Some("")),
            "oauth2"
        );
    }

    #[test]
    fn auth_username_unknown_host_defaults_oauth2() {
        assert_eq!(
            GitService::auth_username_for("https://gitlab.com/x/y.git", None),
            "oauth2"
        );
        // Unparseable URL also falls back to the generic default.
        assert_eq!(GitService::auth_username_for("not a url", None), "oauth2");
        // A spoof host that merely CONTAINS "github.com" must NOT be classified
        // as GitHub (exact/subdomain match only).
        assert_eq!(
            GitService::auth_username_for("https://github.com.evil.example/x/y", None),
            "oauth2"
        );
        // A genuine GitHub subdomain still classifies as GitHub.
        assert_eq!(
            GitService::auth_username_for("https://gist.github.com/x/y", None),
            "x-access-token"
        );
    }
}
