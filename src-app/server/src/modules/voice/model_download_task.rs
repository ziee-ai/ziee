//! whisper-MODEL download tasks + SSE progress events.
//!
//! Model analog of `voice::runtime_version::download_task` (which downloads the
//! whisper-server BINARY). A per-target-filename entry in a DashMap registry, each
//! owning a `broadcast` Sender pumping `Progress`/`Complete`/`Failed` to SSE
//! subscribers, PLUS a per-download cancel flag so the admin can abort an in-flight
//! model download (the binary path only has the global SHUTDOWN race). On success
//! the task upserts a `voice_models` row + emits `sync:voice_model`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};
use crate::modules::voice::model::{self, ModelDownloadSpec};
use crate::modules::voice::models::VoiceModelSource;
use crate::modules::voice::permissions::VoiceAdminRead;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelDownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

impl ModelDownloadStatus {
    fn is_terminal(self) -> bool {
        matches!(self, ModelDownloadStatus::Completed | ModelDownloadStatus::Failed)
    }
}

pub struct ModelDownloadTask {
    pub task_id: Uuid,
    pub key: String,
    pub name: String,
    #[allow(dead_code)]
    pub started_at: SystemTime,
    pub state: Mutex<ModelTaskState>,
    pub events: broadcast::Sender<SSEModelDownloadEvent>,
    /// Set by `cancel()`; the runner checks it each chunk and tears down.
    pub cancelled: Arc<AtomicBool>,
}

pub struct ModelTaskState {
    pub status: ModelDownloadStatus,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub progress: Vec<SSEModelDownloadProgressData>,
    pub error: Option<String>,
    /// The terminal Complete payload, retained so a subscriber that connects
    /// AFTER completion gets the Complete event + a stream close (instead of
    /// hanging in `rx.recv()` forever on the already-fired broadcast).
    pub complete: Option<SSEModelDownloadCompleteData>,
}

// ─────────────────────────── SSE event surface ───────────────────────────

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEModelDownloadConnectedData {
    pub task_id: Uuid,
    pub key: String,
    pub name: String,
    pub status: ModelDownloadStatus,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEModelDownloadProgressData {
    pub status: ModelDownloadStatus,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEModelDownloadCompleteData {
    pub model_id: Uuid,
    pub name: String,
    pub verified: bool,
    pub bytes_downloaded: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEModelDownloadFailedData {
    pub error: String,
}

crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEModelDownloadEvent {
        Connected(SSEModelDownloadConnectedData),
        Progress(SSEModelDownloadProgressData),
        Complete(SSEModelDownloadCompleteData),
        Failed(SSEModelDownloadFailedData),
    }
}

// ─────────────────────────────── Registry ────────────────────────────────

pub static MODEL_DOWNLOAD_TASKS: Lazy<DashMap<String, Arc<ModelDownloadTask>>> =
    Lazy::new(DashMap::new);

static START_OR_JOIN_LOCK: Mutex<()> = Mutex::const_new(());

/// Global graceful-shutdown signal (mirrors the binary download task).
static SHUTDOWN: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);

/// Signal every in-flight model download to interrupt + tear down. Returns the
/// count still running. Called from main.rs on shutdown.
#[allow(dead_code)]
pub async fn shutdown_all() -> usize {
    let mut count = 0usize;
    for entry in MODEL_DOWNLOAD_TASKS.iter() {
        if !entry.value().state.lock().await.status.is_terminal() {
            entry.value().cancelled.store(true, Ordering::Relaxed);
            count += 1;
        }
    }
    SHUTDOWN.notify_waiters();
    count
}

/// The registry key for a model download is its target filename (one download per
/// installed file).
pub fn task_key(filename: &str) -> String {
    filename.to_string()
}

pub fn get_task(key: &str) -> Option<Arc<ModelDownloadTask>> {
    MODEL_DOWNLOAD_TASKS.get(key).map(|e| e.value().clone())
}

/// Request cancellation of an in-flight download by key. Returns true if a
/// non-terminal task was found + signalled.
pub async fn cancel(key: &str) -> bool {
    if let Some(task) = get_task(key) {
        if !task.state.lock().await.status.is_terminal() {
            task.cancelled.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

// ─────────────────────────── Start (or join) ─────────────────────────────

pub async fn start_or_join(spec: ModelDownloadSpec) -> Result<Arc<ModelDownloadTask>, AppError> {
    let key = task_key(&spec.filename);
    let _serialize = START_OR_JOIN_LOCK.lock().await;

    if let Some(existing) = MODEL_DOWNLOAD_TASKS.get(&key).map(|e| e.value().clone()) {
        if !existing.state.lock().await.status.is_terminal() {
            return Ok(existing);
        }
    }
    prune_terminal_tasks(&key).await;
    let task = spawn_runner(spec, &key);
    MODEL_DOWNLOAD_TASKS.insert(key, task.clone());
    Ok(task)
}

/// Bound the registry: terminal (completed/failed) tasks are kept only so a late
/// subscriber can replay the outcome. When the registry grows past a cap, evict
/// terminal entries (never the key being (re)started, never a live task) so it
/// can't grow unbounded across distinct filenames for the process lifetime.
const REGISTRY_CAP: usize = 32;
async fn prune_terminal_tasks(keep_key: &str) {
    if MODEL_DOWNLOAD_TASKS.len() <= REGISTRY_CAP {
        return;
    }
    let mut evict = Vec::new();
    for entry in MODEL_DOWNLOAD_TASKS.iter() {
        if entry.key() == keep_key {
            continue;
        }
        if entry.value().state.lock().await.status.is_terminal() {
            evict.push(entry.key().clone());
        }
    }
    for k in evict {
        MODEL_DOWNLOAD_TASKS.remove(&k);
    }
}

fn spawn_runner(spec: ModelDownloadSpec, key: &str) -> Arc<ModelDownloadTask> {
    let (tx, _rx) = broadcast::channel(64);
    let task = Arc::new(ModelDownloadTask {
        task_id: Uuid::new_v4(),
        key: key.to_string(),
        name: spec.name.clone(),
        started_at: SystemTime::now(),
        state: Mutex::new(ModelTaskState {
            status: ModelDownloadStatus::Pending,
            bytes_received: 0,
            total_bytes: None,
            progress: Vec::new(),
            error: None,
            complete: None,
        }),
        events: tx,
        cancelled: Arc::new(AtomicBool::new(false)),
    });

    let runner_task = task.clone();
    let panic_task = task.clone();
    tokio::spawn(async move {
        let inner = tokio::spawn(async move { run_download(runner_task, spec).await });
        if let Err(join_err) = inner.await {
            let msg = format!("voice model download task ended abnormally: {join_err}");
            tracing::error!("{msg}");
            let mut guard = panic_task.state.lock().await;
            if !guard.status.is_terminal() {
                guard.status = ModelDownloadStatus::Failed;
                guard.error = Some(msg.clone());
                let _ = panic_task
                    .events
                    .send(SSEModelDownloadEvent::Failed(SSEModelDownloadFailedData { error: msg }));
            }
        }
    });
    task
}

async fn run_download(task: Arc<ModelDownloadTask>, spec: ModelDownloadSpec) {
    let source = spec_source(&spec);
    let source_url = spec.url.clone();
    let name = spec.name.clone();

    let task_for_cb = task.clone();
    let cb = move |received: u64, total: Option<u64>| {
        let percent = total.map(|t| if t == 0 { 0.0 } else { (received as f32 / t as f32) * 100.0 });
        if let Ok(mut guard) = task_for_cb.state.try_lock() {
            guard.status = ModelDownloadStatus::Downloading;
            guard.bytes_received = received;
            guard.total_bytes = total;
            const REPLAY_CAP: usize = 32;
            if guard.progress.len() >= REPLAY_CAP {
                guard.progress.remove(0);
            }
            guard.progress.push(SSEModelDownloadProgressData {
                status: ModelDownloadStatus::Downloading,
                bytes_received: received,
                total_bytes: total,
                percent,
            });
        }
        let _ = task_for_cb.events.send(SSEModelDownloadEvent::Progress(
            SSEModelDownloadProgressData {
                status: ModelDownloadStatus::Downloading,
                bytes_received: received,
                total_bytes: total,
                percent,
            },
        ));
    };

    let cancelled = task.cancelled.clone();
    // Race the download against the global shutdown signal (which also sets the
    // per-task cancel flag so the temp file is cleaned up).
    let result = {
        let dl = model::download_model_file(&spec, cb, &cancelled);
        tokio::select! {
            r = dl => r,
            _ = SHUTDOWN.notified() => {
                cancelled.store(true, Ordering::Relaxed);
                Err(AppError::internal_error("model download interrupted by server shutdown"))
            }
        }
    };

    let mut guard = task.state.lock().await;
    match result {
        Ok(dl) => {
            // Register (or update) the installed-model row.
            match Repos
                .voice_model
                .upsert(
                    &name,
                    &dl.filename,
                    source,
                    if matches!(source, VoiceModelSource::Upload) { None } else { Some(source_url.as_str()) },
                    dl.size_bytes as i64,
                    Some(&dl.sha256),
                    dl.verified,
                )
                .await
            {
                Ok(row) => {
                    guard.status = ModelDownloadStatus::Completed;
                    let complete = SSEModelDownloadCompleteData {
                        model_id: row.id,
                        name: row.name,
                        verified: dl.verified,
                        bytes_downloaded: dl.size_bytes,
                    };
                    guard.complete = Some(complete.clone());
                    let _ = task.events.send(SSEModelDownloadEvent::Complete(complete));
                    sync_publish(
                        SyncEntity::VoiceModel,
                        SyncAction::Create,
                        row.id,
                        Audience::perm::<VoiceAdminRead>(),
                        None,
                    );
                }
                Err(e) => fail(&task, &mut guard, e.to_string()),
            }
        }
        Err(e) => fail(&task, &mut guard, e.to_string()),
    }
}

fn fail(task: &Arc<ModelDownloadTask>, guard: &mut ModelTaskState, msg: String) {
    guard.status = ModelDownloadStatus::Failed;
    guard.error = Some(msg.clone());
    let _ = task
        .events
        .send(SSEModelDownloadEvent::Failed(SSEModelDownloadFailedData { error: msg }));
}

fn spec_source(spec: &ModelDownloadSpec) -> VoiceModelSource {
    // A verified (pinned/oid) download is a catalog/HF fetch; an unverified remote
    // fetch is an arbitrary URL. (Uploads never go through this task.)
    if spec.expected_sha256.is_some() {
        VoiceModelSource::Catalog
    } else {
        VoiceModelSource::Url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(key: &str, status: ModelDownloadStatus) -> Arc<ModelDownloadTask> {
        let (tx, _rx) = broadcast::channel(4);
        Arc::new(ModelDownloadTask {
            task_id: Uuid::new_v4(),
            key: key.to_string(),
            name: "n".to_string(),
            started_at: SystemTime::now(),
            state: Mutex::new(ModelTaskState {
                status,
                bytes_received: 0,
                total_bytes: None,
                progress: Vec::new(),
                error: None,
                complete: None,
            }),
            events: tx,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    #[tokio::test]
    async fn cancel_sets_the_flag_on_a_live_task_only() {
        let key = format!("ggml-{}.bin", Uuid::new_v4());
        let live = fake(&key, ModelDownloadStatus::Downloading);
        MODEL_DOWNLOAD_TASKS.insert(key.clone(), live.clone());
        assert!(cancel(&key).await, "a live task is cancellable");
        assert!(live.cancelled.load(Ordering::Relaxed));

        let key2 = format!("ggml-{}.bin", Uuid::new_v4());
        MODEL_DOWNLOAD_TASKS.insert(key2.clone(), fake(&key2, ModelDownloadStatus::Completed));
        assert!(!cancel(&key2).await, "a terminal task is not cancellable");

        MODEL_DOWNLOAD_TASKS.remove(&key);
        MODEL_DOWNLOAD_TASKS.remove(&key2);
    }
}
