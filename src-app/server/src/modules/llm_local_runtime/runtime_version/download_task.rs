//! Engine-binary download tasks + SSE progress events.
//!
//! Mirrors the `code_sandbox::prefetch` pattern: a per-(engine,
//! version, backend) entry in a DashMap registry, each owning a
//! `tokio::sync::broadcast` Sender that pumps `Progress` /
//! `Complete` / `Failed` events to any number of SSE subscribers.
//! Terminal tasks stay in the registry until the NEXT POST for the
//! same key replaces them, so a late subscriber still sees the
//! final outcome rather than a "no task" 404.
//!
//! The `POST /local-runtime/versions/download` handler now runs
//! through `start_or_join`; the inline `Available versions` button
//! in the UI opens the SSE BEFORE issuing the POST so it doesn't
//! miss bytes on a fast/cached download.
//!
//! Keys use a `engine@version@backend` string so DashMap can hash
//! the whole identity in one lookup. Two concurrent POSTs for the
//! same key share one runner via DashMap's `entry`-locked insert
//! (the prefetch module uses the same trick).
//!
//! Progress phases (`Phase`) match the BinaryDownloader sub-steps
//! and surface to the UI for an informative status line under the
//! progress bar.

use std::sync::Arc;
use std::time::SystemTime;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::common::r#type::AppError;
use crate::modules::llm_local_runtime::engine::EngineType;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};
use crate::modules::llm_local_runtime::permissions::RuntimeVersionRead;
use crate::modules::llm_local_runtime::runtime_version::models::RuntimeVersion;
use crate::modules::llm_local_runtime::BinaryManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EngineDownloadStatus {
    Pending,
    Downloading,
    #[allow(dead_code)]
    Verifying,
    #[allow(dead_code)]
    Extracting,
    #[allow(dead_code)]
    Registering,
    Completed,
    Failed,
}

impl EngineDownloadStatus {
    fn is_terminal(self) -> bool {
        matches!(self, EngineDownloadStatus::Completed | EngineDownloadStatus::Failed)
    }
}

/// One in-flight (or terminal) download. Held by an `Arc` so the
/// runner future, the registry entry, and every SSE subscriber can
/// share ownership without copies.
pub struct DownloadTask {
    pub task_id: Uuid,
    pub key: String,
    pub engine: String,
    pub version: String,
    pub backend: String,
    #[allow(dead_code)]
    pub started_at: SystemTime,
    pub state: Mutex<TaskRuntimeState>,
    pub events: broadcast::Sender<SSEEngineDownloadEvent>,
}

pub struct TaskRuntimeState {
    pub status: EngineDownloadStatus,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    /// Replay buffer for late subscribers — last N progress frames
    /// so the UI can repaint without waiting for the next chunk.
    pub progress: Vec<SSEEngineDownloadProgressData>,
    pub result: Option<RuntimeVersion>,
    pub error: Option<String>,
}

// =====================================================================
// SSE event surface
// =====================================================================

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEEngineDownloadConnectedData {
    pub task_id: Uuid,
    pub key: String,
    pub engine: String,
    pub version: String,
    pub backend: String,
    pub status: EngineDownloadStatus,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEEngineDownloadProgressData {
    pub status: EngineDownloadStatus,
    pub bytes_received: u64,
    /// May be `None` when the upstream omits Content-Length (rare
    /// for GitHub Releases). The UI shows an indeterminate bar.
    pub total_bytes: Option<u64>,
    /// `None` when `total_bytes` is None; otherwise 0.0..=100.0.
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEEngineDownloadCompleteData {
    pub version_id: Uuid,
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEEngineDownloadFailedData {
    pub error: String,
}

crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEEngineDownloadEvent {
        Connected(SSEEngineDownloadConnectedData),
        Progress(SSEEngineDownloadProgressData),
        Complete(SSEEngineDownloadCompleteData),
        Failed(SSEEngineDownloadFailedData),
    }
}

// =====================================================================
// Registry
// =====================================================================

pub static DOWNLOAD_TASKS: Lazy<DashMap<String, Arc<DownloadTask>>> =
    Lazy::new(DashMap::new);

/// Engine-download graceful-shutdown signal. Engine-binary downloads (unlike
/// model-file downloads, which use `utils::cancellation::CANCELLATION_TRACKER`)
/// run as detached tasks with no cancellation hook, so on server shutdown they
/// were abruptly aborted mid-transfer — leaving SSE subscribers hanging and the
/// registry entry stuck in `Downloading`. Each runner now races its download
/// against this `Notify`; `shutdown_all()` (called from `main::shutdown_signal`)
/// wakes every in-flight runner so it tears down cleanly (Failed + SSE Failed).
static SHUTDOWN: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);

/// Signal every in-flight engine download to interrupt and tear down. Returns
/// the number of non-terminal tasks that were still running.
#[allow(dead_code)] // called from main.rs (binary crate); invisible to the lib's dead-code analysis
pub async fn shutdown_all() -> usize {
    let mut count = 0usize;
    for entry in DOWNLOAD_TASKS.iter() {
        if !entry.value().state.lock().await.status.is_terminal() {
            count += 1;
        }
    }
    SHUTDOWN.notify_waiters();
    count
}

pub fn task_key(engine: &str, version: &str, backend: &str) -> String {
    format!("{engine}@{version}@{backend}")
}

pub fn get_task(key: &str) -> Option<Arc<DownloadTask>> {
    DOWNLOAD_TASKS.get(key).map(|e| e.value().clone())
}

// =====================================================================
// Start (or join) a download task
// =====================================================================

#[allow(clippy::too_many_arguments)]
pub async fn start_or_join(
    binary_manager: Arc<BinaryManager>,
    engine: EngineType,
    version: String,
    platform: String,
    arch: String,
    backend: String,
) -> Result<Arc<DownloadTask>, AppError> {
    let engine_str = match engine {
        EngineType::Llamacpp => "llamacpp",
        EngineType::Mistralrs => "mistralrs",
    };
    let key = task_key(engine_str, &version, &backend);

    let cell = DOWNLOAD_TASKS
        .entry(key.clone())
        .or_insert_with(|| {
            spawn_runner(
                binary_manager.clone(),
                engine,
                engine_str,
                &version,
                &platform,
                &arch,
                &backend,
                &key,
            )
        })
        .clone();

    // Replace a stuck terminal entry on a re-POST so the user can
    // retry a failed download without restarting the server. The
    // races covered by DashMap's entry-lock above only matter for
    // the *insert* path; this read-then-replace is fine — concurrent
    // re-POSTs converge on whichever replacement landed last and
    // they all join the same runner.
    let status = cell.state.lock().await.status;
    if status.is_terminal() {
        let replacement = spawn_runner(
            binary_manager,
            engine,
            engine_str,
            &version,
            &platform,
            &arch,
            &backend,
            &key,
        );
        DOWNLOAD_TASKS.insert(key, replacement.clone());
        return Ok(replacement);
    }
    Ok(cell)
}

#[allow(clippy::too_many_arguments)]
fn spawn_runner(
    binary_manager: Arc<BinaryManager>,
    engine: EngineType,
    engine_str: &str,
    version: &str,
    platform: &str,
    arch: &str,
    backend: &str,
    key: &str,
) -> Arc<DownloadTask> {
    let (tx, _rx) = broadcast::channel(64);
    let task = Arc::new(DownloadTask {
        task_id: Uuid::new_v4(),
        key: key.to_string(),
        engine: engine_str.to_string(),
        version: version.to_string(),
        backend: backend.to_string(),
        started_at: SystemTime::now(),
        state: Mutex::new(TaskRuntimeState {
            status: EngineDownloadStatus::Pending,
            bytes_received: 0,
            total_bytes: None,
            progress: Vec::new(),
            result: None,
            error: None,
        }),
        events: tx,
    });

    let runner_task = task.clone();
    let version_owned = version.to_string();
    let platform_owned = platform.to_string();
    let arch_owned = arch.to_string();
    let backend_owned = backend.to_string();
    let panic_task = task.clone();
    tokio::spawn(async move {
        // Run the download in an inner task so a panic inside
        // `run_download` (or its progress callback) is observable via the
        // JoinHandle instead of being silently swallowed by the runtime.
        // Without this, a panic would leave the task stuck in
        // `Downloading` forever and every SSE subscriber would hang.
        let inner = tokio::spawn(async move {
            run_download(
                runner_task,
                binary_manager,
                engine,
                version_owned,
                platform_owned,
                arch_owned,
                backend_owned,
            )
            .await;
        });
        if let Err(join_err) = inner.await {
            let msg = if join_err.is_panic() {
                format!("engine download task panicked: {join_err}")
            } else {
                format!("engine download task aborted: {join_err}")
            };
            tracing::error!("{msg}");
            // Reconcile terminal state so subscribers stop waiting. Only
            // overwrite a still-in-flight status (a normal Completed/Failed
            // result already reflects reality).
            let mut guard = panic_task.state.lock().await;
            if !matches!(
                guard.status,
                EngineDownloadStatus::Completed | EngineDownloadStatus::Failed
            ) {
                guard.status = EngineDownloadStatus::Failed;
                guard.error = Some(msg.clone());
                let _ = panic_task.events.send(SSEEngineDownloadEvent::Failed(
                    SSEEngineDownloadFailedData { error: msg },
                ));
            }
        }
    });
    task
}

async fn run_download(
    task: Arc<DownloadTask>,
    binary_manager: Arc<BinaryManager>,
    engine: EngineType,
    version: String,
    platform: String,
    arch: String,
    backend: String,
) {
    let started = std::time::Instant::now();
    let task_for_cb = task.clone();
    let progress_cb = move |received: u64, total: Option<u64>| {
        let percent =
            total.map(|t| if t == 0 { 0.0 } else { (received as f32 / t as f32) * 100.0 });
        // Synchronous callback fired from inside the download
        // future. `try_lock` keeps us non-blocking; the lock is
        // uncontended outside the SSE-subscribe path.
        if let Ok(mut guard) = task_for_cb.state.try_lock() {
            guard.status = EngineDownloadStatus::Downloading;
            guard.bytes_received = received;
            guard.total_bytes = total;
            // Cap the replay buffer so a long download doesn't grow
            // memory unboundedly; the latest N frames are enough
            // for a late subscriber to render a sane initial state.
            const REPLAY_CAP: usize = 32;
            if guard.progress.len() >= REPLAY_CAP {
                guard.progress.remove(0);
            }
            guard.progress.push(SSEEngineDownloadProgressData {
                status: EngineDownloadStatus::Downloading,
                bytes_received: received,
                total_bytes: total,
                percent,
            });
        }
        let _ = task_for_cb.events.send(SSEEngineDownloadEvent::Progress(
            SSEEngineDownloadProgressData {
                status: EngineDownloadStatus::Downloading,
                bytes_received: received,
                total_bytes: total,
                percent,
            },
        ));
    };

    // Race the download against the graceful-shutdown signal so a server stop
    // interrupts the transfer instead of being abruptly aborted with the
    // runtime (which would strand SSE subscribers + the registry entry).
    let result = {
        let download = binary_manager.download_and_register_with_progress(
            engine,
            &version,
            &platform,
            &arch,
            &backend,
            progress_cb,
        );
        tokio::select! {
            r = download => Some(r),
            _ = SHUTDOWN.notified() => None,
        }
    };

    let mut guard = task.state.lock().await;
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        None => {
            let msg = "engine download interrupted by server shutdown".to_string();
            guard.status = EngineDownloadStatus::Failed;
            guard.error = Some(msg.clone());
            let _ = task.events.send(SSEEngineDownloadEvent::Failed(
                SSEEngineDownloadFailedData { error: msg },
            ));
        }
        Some(Ok(version_row)) => {
            let bytes = guard.bytes_received;
            guard.status = EngineDownloadStatus::Completed;
            guard.result = Some(version_row.clone());
            let _ = task.events.send(SSEEngineDownloadEvent::Complete(
                SSEEngineDownloadCompleteData {
                    version_id: version_row.id,
                    bytes_downloaded: bytes,
                    duration_ms,
                },
            ));
            // Realtime sync: the version row now exists — notify admin
            // devices (background task, so no originating connection).
            sync_publish(
                SyncEntity::RuntimeVersion,
                SyncAction::Create,
                version_row.id,
                Audience::perm::<RuntimeVersionRead>(),
                None,
            );
        }
        Some(Err(e)) => {
            let msg = e.to_string();
            guard.status = EngineDownloadStatus::Failed;
            guard.error = Some(msg.clone());
            let _ = task.events.send(SSEEngineDownloadEvent::Failed(
                SSEEngineDownloadFailedData { error: msg },
            ));
        }
    }
    // Dropping the last strong ref to `task.events` closes the
    // broadcast and signals every SSE subscriber to exit. But the
    // registry still holds an Arc to `task`, so the Sender lives on
    // until the entry is replaced by the next POST — which is what
    // lets late subscribers replay terminal state.
}
