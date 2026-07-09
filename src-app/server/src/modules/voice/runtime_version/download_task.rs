//! whisper-server binary download tasks + SSE progress events.
//!
//! Single-engine analog of `llm_local_runtime::runtime_version::download_task`.
//! A per-(version, backend) entry in a DashMap registry, each owning a
//! `tokio::sync::broadcast` Sender that pumps `Progress` / `Complete` /
//! `Failed` events to any number of SSE subscribers. Terminal tasks stay in the
//! registry until the NEXT POST for the same key replaces them, so a late
//! subscriber still sees the final outcome rather than a "no task" 404.
//!
//! Keys are `whisper@{version}@{backend}` (the engine segment is constant but
//! kept so the key format matches the LLM runtime + reads unambiguously).
//!
//! Unlike the LLM runtime this task owns its own download+register logic
//! (voice's `binary_manager` is a set of free functions, not a struct handle):
//! it drives `WhisperDownloader` + the version repository directly.

use std::sync::Arc;
use std::time::SystemTime;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};
use crate::modules::voice::engine::WhisperDownloader;
use crate::modules::voice::permissions::VoiceAdminRead;
use crate::modules::voice::runtime_version::models::RuntimeVersion;
use crate::modules::voice::runtime_version::repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EngineDownloadStatus {
    Pending,
    Downloading,
    // Intermediate phases retained for API completeness / SSE contract parity.
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
        matches!(
            self,
            EngineDownloadStatus::Completed | EngineDownloadStatus::Failed
        )
    }
}

/// One in-flight (or terminal) download. Held by an `Arc` so the runner future,
/// the registry entry, and every SSE subscriber share ownership without copies.
pub struct DownloadTask {
    pub task_id: Uuid,
    pub key: String,
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
    /// Replay buffer for late subscribers.
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
    pub version: String,
    pub backend: String,
    pub status: EngineDownloadStatus,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEEngineDownloadProgressData {
    pub status: EngineDownloadStatus,
    pub bytes_received: u64,
    /// `None` when the upstream omits Content-Length.
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

pub static DOWNLOAD_TASKS: Lazy<DashMap<String, Arc<DownloadTask>>> = Lazy::new(DashMap::new);

/// Graceful-shutdown signal. Each runner races its download against this
/// `Notify`; `shutdown_all()` wakes every in-flight runner so it tears down
/// cleanly (Failed + SSE Failed) instead of being abruptly aborted mid-transfer.
static SHUTDOWN: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);

/// Signal every in-flight whisper download to interrupt + tear down. Returns
/// the number of non-terminal tasks still running.
// Called from main.rs (binary crate); invisible to the lib's dead-code analysis.
#[allow(dead_code)]
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

pub fn task_key(version: &str, backend: &str) -> String {
    format!("whisper@{version}@{backend}")
}

pub fn get_task(key: &str) -> Option<Arc<DownloadTask>> {
    DOWNLOAD_TASKS.get(key).map(|e| e.value().clone())
}

// =====================================================================
// Start (or join) a download task
// =====================================================================

pub async fn start_or_join(
    version: String,
    platform: String,
    arch: String,
    backend: String,
) -> Result<Arc<DownloadTask>, AppError> {
    let key = task_key(&version, &backend);

    let cell = DOWNLOAD_TASKS
        .entry(key.clone())
        .or_insert_with(|| spawn_runner(&version, &platform, &arch, &backend, &key))
        .clone();

    // Replace a stuck terminal entry on a re-POST so a failed download can be
    // retried without restarting the server.
    let status = cell.state.lock().await.status;
    if status.is_terminal() {
        let replacement = spawn_runner(&version, &platform, &arch, &backend, &key);
        DOWNLOAD_TASKS.insert(key, replacement.clone());
        return Ok(replacement);
    }
    Ok(cell)
}

fn spawn_runner(
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
        // Run inside an inner task so a panic in `run_download` (or its progress
        // callback) is observable via the JoinHandle instead of leaving the
        // task stuck in `Downloading` forever with every subscriber hung.
        let inner = tokio::spawn(async move {
            run_download(
                runner_task,
                version_owned,
                platform_owned,
                arch_owned,
                backend_owned,
            )
            .await;
        });
        if let Err(join_err) = inner.await {
            let msg = if join_err.is_panic() {
                format!("whisper download task panicked: {join_err}")
            } else {
                format!("whisper download task aborted: {join_err}")
            };
            tracing::error!("{msg}");
            let mut guard = panic_task.state.lock().await;
            if !guard.status.is_terminal() {
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
        if let Ok(mut guard) = task_for_cb.state.try_lock() {
            guard.status = EngineDownloadStatus::Downloading;
            guard.bytes_received = received;
            guard.total_bytes = total;
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
        let _ = task_for_cb
            .events
            .send(SSEEngineDownloadEvent::Progress(SSEEngineDownloadProgressData {
                status: EngineDownloadStatus::Downloading,
                bytes_received: received,
                total_bytes: total,
                percent,
            }));
    };

    // Race the download against graceful shutdown.
    let result = {
        let download = download_and_register(&version, &platform, &arch, &backend, progress_cb);
        tokio::select! {
            r = download => Some(r),
            _ = SHUTDOWN.notified() => None,
        }
    };

    let mut guard = task.state.lock().await;
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        None => {
            let msg = "whisper download interrupted by server shutdown".to_string();
            guard.status = EngineDownloadStatus::Failed;
            guard.error = Some(msg.clone());
            let _ = task
                .events
                .send(SSEEngineDownloadEvent::Failed(SSEEngineDownloadFailedData {
                    error: msg,
                }));
        }
        Some(Ok(version_row)) => {
            let bytes = guard.bytes_received;
            guard.status = EngineDownloadStatus::Completed;
            guard.result = Some(version_row.clone());
            let _ = task
                .events
                .send(SSEEngineDownloadEvent::Complete(SSEEngineDownloadCompleteData {
                    version_id: version_row.id,
                    bytes_downloaded: bytes,
                    duration_ms,
                }));
            // Realtime sync: the version row now exists — notify admin devices
            // (background task, so no originating connection). Notify-and-refetch
            // (the client refetches the version list), so a nil id is sufficient.
            sync_publish(
                SyncEntity::VoiceRuntimeVersion,
                SyncAction::Create,
                Uuid::nil(),
                Audience::perm::<VoiceAdminRead>(),
                None,
            );
        }
        Some(Err(e)) => {
            let msg = e.to_string();
            guard.status = EngineDownloadStatus::Failed;
            guard.error = Some(msg.clone());
            let _ = task
                .events
                .send(SSEEngineDownloadEvent::Failed(SSEEngineDownloadFailedData {
                    error: msg,
                }));
        }
    }
}

/// Download the whisper-server binary and register (or dedupe to an existing)
/// version row. Returns the persisted [`RuntimeVersion`].
async fn download_and_register<F>(
    version: &str,
    platform: &str,
    arch: &str,
    backend: &str,
    progress: F,
) -> Result<RuntimeVersion, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(u64, Option<u64>) + Send + Sync,
{
    let downloader = WhisperDownloader::new()?;
    let info = downloader
        .download_with_progress(version, platform, arch, backend, progress)
        .await?;

    // Dedupe: if this identity already exists (e.g. cached + previously
    // registered), return it instead of inserting a duplicate.
    let pool = crate::core::Repos.pool();
    if let Some(existing) =
        repository::get_by_identity(pool, &info.version, platform, arch, backend).await?
    {
        tracing::info!(
            "whisper runtime version already registered: {} {}-{}-{}",
            info.version,
            platform,
            arch,
            backend
        );
        return Ok(existing);
    }

    let row = repository::create(
        pool,
        &info.version,
        platform,
        arch,
        backend,
        info.path.to_string_lossy().as_ref(),
    )
    .await?;
    tracing::info!(
        "Registered whisper runtime version: {} ({})",
        info.version,
        row.id
    );
    Ok(row)
}
