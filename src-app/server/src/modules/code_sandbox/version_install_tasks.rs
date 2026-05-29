//! In-memory task registry + SSE broadcast for rootfs version
//! installs (Plan 5 Phase 2c — SSE port).
//!
//! Mirrors the `llm_model::handlers::downloads` + `hardware`
//! monitoring patterns:
//!   - `POST /code-sandbox/rootfs/versions/install` spawns the
//!     install in a background tokio task, returns a task_id
//!     immediately (HTTP 202).
//!   - `GET  /code-sandbox/rootfs/versions/install/subscribe` opens an
//!     SSE stream emitting typed `SSEInstallTaskEvent`s for every
//!     active task. The aide-generated TypeScript client gets the
//!     enum + payload types for free.
//!
//! The admin UI subscribes once on mount; each `Install` button click
//! arrives via the POST endpoint and the UI watches its task_id
//! through the already-open SSE stream.

use axum::response::sse::Event;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::modules::code_sandbox::version_manager::{self, InstallProgress};

type ClientId = Uuid;
type ClientSender = UnboundedSender<Result<Event, axum::Error>>;

/// Connected SSE clients, keyed by client_id. Each entry receives
/// every install-task event. Cleanup happens on `send` failure
/// (client dropped).
pub static SSE_CLIENTS: Lazy<Mutex<HashMap<ClientId, ClientSender>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Active + recently-completed install tasks, keyed by task_id. The
/// spawned install task cleans up its own cell `TASK_RETENTION` after
/// the terminal event so reconnects can replay recent outcomes
/// without the map growing unbounded.
pub static INSTALL_TASKS: Lazy<DashMap<Uuid, Arc<Mutex<InstallTaskState>>>> =
    Lazy::new(DashMap::new);

const TASK_RETENTION: Duration = Duration::from_secs(5 * 60);

/// Audit Net1: cap concurrent SSE subscribers to bound memory + per-
/// broadcast send work. Each connected client costs one
/// `UnboundedSender` slot in `SSE_CLIENTS` plus a per-event clone of
/// every event. A malicious or buggy client that reconnects in a tight
/// loop without cleaning up could otherwise exhaust the map. 256 is
/// comfortably above the operator's UI tab + curl-debugging needs.
pub const MAX_SSE_CLIENTS: usize = 256;

// =====================================================================
// Typed SSE event payloads
// =====================================================================

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEInstallConnectedData {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEInstallProgressData {
    pub task_id: Uuid,
    pub phase: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEInstallCompleteData {
    pub task_id: Uuid,
    pub artifact_id: Uuid,
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
    pub cosign_verified: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEInstallFailedData {
    pub task_id: Uuid,
    pub error: String,
}

// `TaskState` variant is replayed on subscribe for every
// currently-known task so a fresh client sees in-flight +
// recently-finished state without waiting for the next tick.
crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEInstallTaskEvent {
        Connected(SSEInstallConnectedData),
        TaskStarted(InstallTaskState),
        Progress(SSEInstallProgressData),
        Complete(SSEInstallCompleteData),
        Failed(SSEInstallFailedData),
        TaskState(InstallTaskState),
    }
}

// =====================================================================
// Task state
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InstallTaskState {
    pub task_id: Uuid,
    pub version: String,
    pub arch: String,
    pub flavor: String,
    pub package: String,
    pub status: TaskStatus,
    pub phase: Option<String>,
    pub message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub artifact_id: Option<Uuid>,
    pub bytes_downloaded: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

impl InstallTaskState {
    fn new(version: &str, arch: &str, flavor: &str, package: &str) -> Self {
        Self {
            task_id: Uuid::new_v4(),
            version: version.to_string(),
            arch: arch.to_string(),
            flavor: flavor.to_string(),
            package: package.to_string(),
            status: TaskStatus::Running,
            phase: None,
            message: None,
            started_at: Utc::now(),
            completed_at: None,
            artifact_id: None,
            bytes_downloaded: None,
            duration_ms: None,
            error: None,
        }
    }
}

// =====================================================================
// Registry queries
// =====================================================================

pub fn list_tasks() -> Vec<InstallTaskState> {
    INSTALL_TASKS
        .iter()
        .filter_map(|e| e.value().lock().ok().map(|g| g.clone()))
        .collect()
}

pub fn get_task(task_id: Uuid) -> Option<InstallTaskState> {
    INSTALL_TASKS
        .get(&task_id)
        .and_then(|e| e.value().lock().ok().map(|g| g.clone()))
}

// =====================================================================
// SSE client lifecycle
// =====================================================================

pub fn register_client(tx: ClientSender) -> Option<ClientId> {
    let id = Uuid::new_v4();
    let mut g = SSE_CLIENTS.lock().ok()?;
    // Audit Net1: refuse new subscribers once cap is hit so a
    // reconnect storm can't blow up server memory.
    if g.len() >= MAX_SSE_CLIENTS {
        tracing::warn!(
            current = g.len(),
            cap = MAX_SSE_CLIENTS,
            "code_sandbox: SSE subscribe rejected — connection cap reached"
        );
        return None;
    }
    g.insert(id, tx);
    Some(id)
}

pub fn remove_client(id: ClientId) {
    if let Ok(mut g) = SSE_CLIENTS.lock() {
        g.remove(&id);
    }
}

/// Send an event to every connected client. Drops senders whose
/// receivers are gone (client disconnected).
fn broadcast(event: SSEInstallTaskEvent) {
    let snapshot: Vec<(ClientId, ClientSender)> = match SSE_CLIENTS.lock() {
        Ok(g) => g.iter().map(|(k, v)| (*k, v.clone())).collect(),
        Err(_) => return,
    };
    let mut dead: Vec<ClientId> = Vec::new();
    let axum_event: Event = event.into();
    for (id, tx) in &snapshot {
        if tx.send(Ok(axum_event.clone())).is_err() {
            dead.push(*id);
        }
    }
    if !dead.is_empty()
        && let Ok(mut g) = SSE_CLIENTS.lock()
    {
        for id in dead {
            g.remove(&id);
        }
    }
}

/// Send a single typed event to ONE client (used by the
/// subscribe handler to replay current registry state on connect).
pub fn send_to(tx: &ClientSender, event: SSEInstallTaskEvent) {
    let axum_event: Event = event.into();
    let _ = tx.send(Ok(axum_event));
}

// =====================================================================
// Install task lifecycle
// =====================================================================

/// Spawn an install task and return its initial state. The actual
/// download runs in `tokio::spawn`; the HTTP handler returns the
/// state immediately so the UI can show "running" while progress
/// flows via the SSE channel.
pub fn start_install_task(
    pool: PgPool,
    cache_dir: PathBuf,
    version: String,
    arch: String,
    flavor: String,
    package: String,
) -> InstallTaskState {
    let state = InstallTaskState::new(&version, &arch, &flavor, &package);
    let task_id = state.task_id;
    let cell = Arc::new(Mutex::new(state.clone()));
    INSTALL_TASKS.insert(task_id, cell.clone());

    broadcast(SSEInstallTaskEvent::TaskStarted(state.clone()));

    tokio::spawn(async move {
        let cell_for_progress = cell.clone();
        let progress_cb = move |ev: InstallProgress| {
            let (phase, message) = match &ev {
                InstallProgress::Resolving { version, asset } => (
                    "resolving".to_string(),
                    format!("resolving v{version} {asset}"),
                ),
                InstallProgress::Downloading { url } => {
                    ("downloading".to_string(), format!("downloading {url}"))
                }
                InstallProgress::VerifyingSha256 => (
                    "verifying_sha256".to_string(),
                    "verifying sha256".to_string(),
                ),
                InstallProgress::VerifyingCosign => (
                    "verifying_cosign".to_string(),
                    "verifying cosign signature".to_string(),
                ),
                InstallProgress::Installing { path } => {
                    ("installing".to_string(), format!("installing {path}"))
                }
            };
            if let Ok(mut g) = cell_for_progress.lock() {
                g.phase = Some(phase.clone());
                g.message = Some(message.clone());
            }
            broadcast(SSEInstallTaskEvent::Progress(SSEInstallProgressData {
                task_id,
                phase,
                message,
            }));
        };

        let result = version_manager::install_version(
            &pool,
            &cache_dir,
            &version,
            &arch,
            &flavor,
            &package,
            progress_cb,
        )
        .await;

        match result {
            Ok((artifact, stats)) => {
                let stats = stats.unwrap_or(version_manager::DownloadStats {
                    bytes_downloaded: 0,
                    duration_ms: 0,
                    cosign_verified: artifact.cosign_bundle.is_some(),
                });
                if let Ok(mut g) = cell.lock() {
                    g.status = TaskStatus::Completed;
                    g.phase = Some("complete".to_string());
                    g.message = Some("installed".to_string());
                    g.completed_at = Some(Utc::now());
                    g.artifact_id = Some(artifact.id);
                    g.bytes_downloaded = Some(stats.bytes_downloaded);
                    g.duration_ms = Some(stats.duration_ms);
                }
                broadcast(SSEInstallTaskEvent::Complete(SSEInstallCompleteData {
                    task_id,
                    artifact_id: artifact.id,
                    bytes_downloaded: stats.bytes_downloaded,
                    duration_ms: stats.duration_ms,
                    cosign_verified: stats.cosign_verified,
                }));
            }
            Err(e) => {
                let err_str = e.to_string();
                if let Ok(mut g) = cell.lock() {
                    g.status = TaskStatus::Failed;
                    g.phase = Some("failed".to_string());
                    g.completed_at = Some(Utc::now());
                    g.error = Some(err_str.clone());
                }
                broadcast(SSEInstallTaskEvent::Failed(SSEInstallFailedData {
                    task_id,
                    error: err_str,
                }));
            }
        }

        // Reap the cell after the retention window. Live clients
        // see the terminal event immediately; this just keeps
        // INSTALL_TASKS from growing unbounded.
        tokio::time::sleep(TASK_RETENTION).await;
        INSTALL_TASKS.remove(&task_id);
    });

    state
}
