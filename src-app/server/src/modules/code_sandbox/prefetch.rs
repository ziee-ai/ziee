//! Admin-triggered rootfs prefetch tasks + SSE event types.
//!
//! Each task is a long-running background job that downloads a rootfs
//! squashfs (via `runtime_fetch::fetch_flavor`) and broadcasts
//! progress events over a `tokio::sync::broadcast` channel so any
//! number of admin-UI SSE subscribers can watch in real time. Tasks
//! are keyed by flavor — at most one running per flavor at a time;
//! a new POST for the same flavor while one is running joins the
//! existing task instead of starting a duplicate.
//!
//! Terminal task state (Completed / Failed / AlreadyCached) stays in
//! the registry until the NEXT POST for the same flavor replaces it.
//! That way a late SSE subscriber can still see the final outcome
//! without missing the broadcast.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use axum::http::StatusCode;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Serialize;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::common::r#type::AppError;
use crate::modules::code_sandbox::runtime_fetch::{
    self, FetchOutcome, FetchPhase, FetchProgress,
};
use crate::modules::code_sandbox::types::KNOWN_FLAVORS;

// =====================================================================
// Public types
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrefetchStatus {
    Running,
    Completed,
    Failed,
    AlreadyCached,
}

impl PrefetchStatus {
    fn is_terminal(self) -> bool {
        !matches!(self, PrefetchStatus::Running)
    }
}

/// One prefetch task. The `events` broadcast Sender lives here for
/// the task's whole lifetime; dropping it on shutdown (when the task
/// future ends) signals every SSE subscriber to close.
pub struct PrefetchTask {
    pub task_id: Uuid,
    pub flavor: String,
    pub started_at: SystemTime,
    pub expected_size_mb: u64,
    pub state: Mutex<TaskRuntimeState>,
    pub events: broadcast::Sender<SSEPrefetchEvent>,
}

pub struct TaskRuntimeState {
    pub status: PrefetchStatus,
    /// Replay buffer for late subscribers.
    pub progress: Vec<FetchProgress>,
    pub outcome: Option<FetchOutcome>,
    pub error: Option<String>,
}

// =====================================================================
// SSE event surface (typed; exposed to OpenAPI as a discriminated union)
// =====================================================================

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEPrefetchConnectedData {
    pub flavor: String,
    pub task_id: Uuid,
    pub status: PrefetchStatus,
    /// Total expected size from `KNOWN_FLAVORS` metadata — lets the
    /// admin UI render a progress-bar denominator before any
    /// download bytes have arrived.
    pub expected_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEPrefetchProgressData {
    pub phase: FetchPhase,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEPrefetchCompleteData {
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
    pub cosign_verified: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEPrefetchFailedData {
    pub error: String,
}

// The macro:
//   - preserves the listed derives (Debug, Clone, Serialize, JsonSchema)
//   - generates `event_name()` returning camelCase variant names
//     ("connected", "progress", "complete", "failed")
//   - generates `data()` serializing the wrapped struct to JSON
//   - generates `impl Into<axum::response::sse::Event>` so handlers
//     can `yield Ok(SSEPrefetchEvent::Progress(...).into())`
//
// The frontend's generated types.ts gets a keyed-union TypeScript
// type matching how SSEHardwareUsageEvent / SSEDownloadProgressEvent
// are exposed today.
crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEPrefetchEvent {
        Connected(SSEPrefetchConnectedData),
        Progress(SSEPrefetchProgressData),
        Complete(SSEPrefetchCompleteData),
        Failed(SSEPrefetchFailedData),
    }
}

// =====================================================================
// Registry
// =====================================================================

pub static PREFETCH_TASKS: Lazy<DashMap<String, Arc<PrefetchTask>>> = Lazy::new(DashMap::new);

pub fn get_task(flavor: &str) -> Option<Arc<PrefetchTask>> {
    PREFETCH_TASKS.get(flavor).map(|e| e.value().clone())
}

pub fn list_tasks() -> Vec<Arc<PrefetchTask>> {
    let mut all: Vec<Arc<PrefetchTask>> =
        PREFETCH_TASKS.iter().map(|e| e.value().clone()).collect();
    all.sort_by(|a, b| a.flavor.cmp(&b.flavor));
    all
}

// =====================================================================
// Start (or join) a prefetch task
// =====================================================================

/// Idempotent: concurrent calls for the same flavor while a task is
/// Running return the same Arc. Calls after terminal status replace
/// the entry with a fresh task.
pub async fn start_or_join(
    cache_dir: &Path,
    flavor: &str,
) -> Result<Arc<PrefetchTask>, AppError> {
    // Validate against KNOWN_FLAVORS — the static compile-time set of
    // flavors this binary knows about. This is the same source the
    // MCP `execute_command` schema enum + REST `list_environments`
    // read from, so all three surfaces agree on what's a valid flavor.
    // (Filtering by `known_revisions.toml` happens later, inside
    // runtime_fetch::fetch_flavor — and that's properly per-revision
    // rather than per-flavor.)
    if !KNOWN_FLAVORS.iter().any(|m| m.flavor == flavor) {
        let available: Vec<&'static str> =
            KNOWN_FLAVORS.iter().map(|m| m.flavor).collect();
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "SANDBOX_UNKNOWN_FLAVOR",
            format!(
                "unknown flavor {flavor:?}; available: {available:?}"
            ),
        ));
    }

    let cache_dir_owned = cache_dir.to_path_buf();
    let flavor_owned = flavor.to_string();

    // Atomic insert-if-absent: DashMap's `entry` takes the shard
    // lock for `flavor`'s key, so only ONE concurrent call sees the
    // vacant slot and runs the closure. Concurrent siblings get the
    // same Arc back without spawning a duplicate runner.
    let cell = PREFETCH_TASKS
        .entry(flavor_owned.clone())
        .or_insert_with(|| spawn_new_task(&flavor_owned, &cache_dir_owned))
        .clone();

    // Already-inserted case: it might be a terminal task left over
    // from a prior run. If so, replace it with a fresh one. (Without
    // this, a previously-failed prefetch would be sticky forever.)
    let snap_status = cell.state.lock().await.status;
    if snap_status.is_terminal() {
        let replacement = spawn_new_task(&flavor_owned, &cache_dir_owned);
        PREFETCH_TASKS.insert(flavor_owned, replacement.clone());
        return Ok(replacement);
    }
    Ok(cell)
}

fn spawn_new_task(flavor: &str, cache_dir: &Path) -> Arc<PrefetchTask> {
    let (tx, _rx) = broadcast::channel(64);
    let expected_size_mb = KNOWN_FLAVORS
        .iter()
        .find(|m| m.flavor == flavor)
        .map(|m| m.approximate_size_mb)
        .unwrap_or(0);
    let task = Arc::new(PrefetchTask {
        task_id: Uuid::new_v4(),
        flavor: flavor.to_string(),
        started_at: SystemTime::now(),
        expected_size_mb,
        state: Mutex::new(TaskRuntimeState {
            status: PrefetchStatus::Running,
            progress: Vec::new(),
            outcome: None,
            error: None,
        }),
        events: tx,
    });
    let runner_task = task.clone();
    let runner_cache = cache_dir.to_path_buf();
    tokio::spawn(async move {
        run_fetch(runner_task, runner_cache).await;
    });
    task
}

async fn run_fetch(task: Arc<PrefetchTask>, cache_dir: std::path::PathBuf) {
    let task_for_cb = task.clone();
    let flavor = task.flavor.clone();
    let progress_cb = move |p: FetchProgress| {
        // The callback runs synchronously on the fetch thread
        // (spawn_blocking inside runtime_fetch::fetch_flavor). We
        // can't await the Mutex here, so use try_lock + fall back to
        // a blocking lock; the lock is uncontended outside the
        // SSE-subscribe path so this is fine.
        if let Ok(mut guard) = task_for_cb.state.try_lock() {
            guard.progress.push(p.clone());
        }
        let ev = SSEPrefetchEvent::Progress(SSEPrefetchProgressData {
            phase: p.phase,
            message: p.message,
        });
        // Ignore send errors — `Err` just means no subscribers right
        // now, which is fine; the replay buffer captures it.
        let _ = task_for_cb.events.send(ev);
    };

    let result = runtime_fetch::fetch_flavor(&cache_dir, &flavor, progress_cb).await;

    let mut guard = task.state.lock().await;
    match result {
        Ok(outcome) => {
            let event = SSEPrefetchEvent::Complete(SSEPrefetchCompleteData {
                bytes_downloaded: outcome.bytes_downloaded,
                duration_ms: outcome.duration_ms,
                cosign_verified: outcome.cosign_verified,
            });
            guard.status = if outcome.bytes_downloaded == 0 {
                PrefetchStatus::AlreadyCached
            } else {
                PrefetchStatus::Completed
            };
            guard.outcome = Some(outcome);
            let _ = task.events.send(event);
        }
        Err(e) => {
            let msg = e.to_string();
            guard.status = PrefetchStatus::Failed;
            guard.error = Some(msg.clone());
            let _ = task.events.send(SSEPrefetchEvent::Failed(SSEPrefetchFailedData {
                error: msg,
            }));
        }
    }
    // `task.events` Sender will be dropped when this task future ends
    // (we hold the last strong reference here). That closes the
    // broadcast channel and signals every SSE subscriber to exit.
}
