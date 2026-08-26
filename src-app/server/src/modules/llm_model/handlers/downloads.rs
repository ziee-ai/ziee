// LLM Model Download Management Handlers
// Source: react-test/src-tauri/src/api/download_instances.rs
// Following ziee patterns with handlers and docs together

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::interval;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::repository::Repos,
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::{
    models::{DownloadInstance, DownloadPhase, DownloadStatus},
    permissions::*,
    types::{DownloadInstanceListResponse, UpdateDownloadStatusRequest},
};

// =====================================================
// Query Types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadPaginationQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub status: Option<String>,
}

// =====================================================
// SSE Event Types
// =====================================================

/// Simplified progress data for SSE streaming
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DownloadProgressUpdate {
    pub id: String,
    pub provider_id: String,
    pub status: String,
    pub phase: DownloadPhase,
    pub current: Option<i64>,
    pub total: Option<i64>,
    pub message: Option<String>,
    pub speed_bps: Option<i64>,
    pub eta_seconds: Option<i64>,
    pub error_message: Option<String>,
    pub model_id: Option<String>,
}

impl From<&DownloadInstance> for DownloadProgressUpdate {
    fn from(download: &DownloadInstance) -> Self {
        DownloadProgressUpdate {
            id: download.id.to_string(),
            provider_id: download.provider_id.to_string(),
            status: download.status.as_str().to_string(),
            phase: download
                .progress_data
                .as_ref()
                .map(|p| p.phase)
                .unwrap_or(DownloadPhase::Created),
            current: download.progress_data.as_ref().map(|p| p.current),
            total: download.progress_data.as_ref().map(|p| p.total),
            message: download
                .progress_data
                .as_ref().map(|p| p.message.clone()),
            speed_bps: download.progress_data.as_ref().map(|p| p.speed_bps),
            eta_seconds: download.progress_data.as_ref().map(|p| p.eta_seconds),
            error_message: download.error_message.clone(),
            model_id: download.model_id.map(|id| id.to_string()),
        }
    }
}

/// SSE connected event data
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEDownloadProgressConnectedData {
    pub message: Option<String>,
}

// SSE event types for download progress
crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEDownloadProgressEvent {
        Connected(SSEDownloadProgressConnectedData),
        Update(Vec<DownloadProgressUpdate>),
        Complete(String),
        Error(String),
    }
}

// =====================================================
// SSE Connection Management
// =====================================================

type ClientId = Uuid;

lazy_static::lazy_static! {
    static ref SSE_CLIENTS: Mutex<HashMap<ClientId, tokio::sync::mpsc::UnboundedSender<Result<Event, axum::Error>>>> = Mutex::new(HashMap::new());
    static ref MONITORING_ACTIVE: Mutex<bool> = Mutex::new(false);
}

// =====================================================
// Download Management Handlers
// =====================================================

/// GET /api/llm-models/downloads
/// List all download instances (paginated, with optional status filter)
#[debug_handler]
pub async fn list_all_downloads(
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    Query(params): Query<DownloadPaginationQuery>,
    
) -> ApiResult<Json<DownloadInstanceListResponse>> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    // Parse status filter if provided
    let status_filter = params
        .status
        .as_ref()
        .and_then(|s| DownloadStatus::from_str(s));

    let response = Repos.download_instance
        .list(page, per_page, status_filter)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get all downloads: {}", e);
            AppError::internal_error("Failed to retrieve downloads").to_api_error()
        })?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn list_all_downloads_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsRead,)>(op)
        .id("LlmModel.listDownloads")
        .tag("LLM Models - Downloads")
        .summary("List all download instances")
        .description("Get paginated list of download instances with optional status filter")
        .response::<200, Json<DownloadInstanceListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/llm-models/downloads/{download_id}
/// Get a specific download instance by ID
#[debug_handler]
pub async fn get_download(
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    Path(download_id): Path<Uuid>,
    
) -> ApiResult<Json<DownloadInstance>> {
    let download = Repos.download_instance
        .get_by_id(download_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get download {}: {}", download_id, e);
            AppError::internal_error("Database operation failed").to_api_error()
        })?
        .ok_or_else(|| AppError::not_found("Download instance").to_api_error())?;

    Ok((StatusCode::OK, Json(download)))
}

pub fn get_download_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsRead,)>(op)
        .id("LlmModel.getDownload")
        .tag("LLM Models - Downloads")
        .summary("Get download instance by ID")
        .description("Retrieve a specific download instance")
        .response::<200, Json<DownloadInstance>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// POST /api/llm-models/downloads/{download_id}/cancel
/// Cancel an active download
#[debug_handler]
pub async fn cancel_download(
    _auth: RequirePermissions<(LlmModelsDownloadsCancel,)>,
    Path(download_id): Path<Uuid>,
    
) -> ApiResult<StatusCode> {
    // Verify the download exists and user has access
    let download = Repos.download_instance
        .get_by_id(download_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to verify download {}: {}", download_id, e);
            AppError::internal_error("Database operation failed").to_api_error()
        })?
        .ok_or_else(|| AppError::not_found("Download instance").to_api_error())?;

    // Check if download can be cancelled
    if !download.can_cancel() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_STATE",
                "Download cannot be cancelled in its current state",
            ),
        ));
    }

    // Signal cancellation to the background download task first
    let cancellation_result = crate::utils::cancellation::cancel_download(download_id).await;

    if cancellation_result {
        tracing::info!(
            "Download {} cancellation signal sent successfully",
            download_id
        );
    } else {
        tracing::warn!(
            "Download {} was not being tracked for cancellation",
            download_id
        );
    }

    // Update status to cancelled first so users can see the cancellation
    let cancel_request = UpdateDownloadStatusRequest {
        status: DownloadStatus::Cancelled,
        error_message: Some("Cancelled by user".to_string()),
        model_id: None,
    };

    let _updated = Repos.download_instance
        .update_status(download_id, cancel_request)
        .await
        .map_err(|e| {
            tracing::error!("Failed to cancel download {}: {}", download_id, e);
            AppError::internal_error("Failed to cancel download").to_api_error()
        })?
        .ok_or_else(|| AppError::not_found("Download instance").to_api_error())?;

    tracing::info!("Download {} marked as cancelled", download_id);

    // Spawn a background task to delete the cancelled download after 60 seconds
    let repo_clone = Repos.download_instance.clone();
    tokio::spawn(async move {
        tracing::info!(
            "Scheduling deletion of cancelled download {} in 60 seconds",
            download_id
        );
        tokio::time::sleep(Duration::from_secs(60)).await;

        match repo_clone.delete(download_id).await {
            Ok(true) => {
                tracing::info!(
                    "Successfully deleted cancelled download {} after 60 seconds",
                    download_id
                );
            }
            Ok(false) => {
                tracing::warn!("Cancelled download {} was already deleted", download_id);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to delete cancelled download {} after 60 seconds: {}",
                    download_id,
                    e
                );
            }
        }
    });

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn cancel_download_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsCancel,)>(op)
        .id("LlmModel.cancelDownload")
        .tag("LLM Models - Downloads")
        .summary("Cancel an active download")
        .description("Cancel a download that is pending or in progress. The download will be automatically deleted after 60 seconds.")
        .response_with::<204, (), _>(|res| res.description("Download cancelled successfully"))
        .response_with::<400, (), _>(|res| res.description("Cannot cancel download in current state"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// DELETE /api/llm-models/downloads/{download_id}
/// Delete a download instance (only terminal states)
#[debug_handler]
pub async fn delete_download(
    _auth: RequirePermissions<(LlmModelsDownloadsDelete,)>,
    Path(download_id): Path<Uuid>,
    
) -> ApiResult<StatusCode> {
    // DELETE is idempotent: a row that is already absent — never existed, or
    // concurrently removed by the boot-time retention prune loop (prune.rs)
    // between this lookup and the delete — yields 204, not a confusing 404. We
    // only 400 when the row is present AND still active.
    let download = Repos.download_instance
        .get_by_id(download_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to verify download {}: {}", download_id, e);
            AppError::internal_error("Database operation failed").to_api_error()
        })?;

    let download = match download {
        Some(d) => d,
        // Already gone — idempotent success.
        None => return Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT)),
    };

    // Only allow deleting terminal states.
    if !download.is_terminal() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_STATE", "Cannot delete active download"),
        ));
    }

    // Ignore the deleted flag: if the row vanished between the lookup above and
    // here (prune loop / concurrent delete), the post-condition "row is gone"
    // still holds, so the DELETE succeeds idempotently.
    Repos.download_instance.delete(download_id).await.map_err(|e| {
        tracing::error!("Failed to delete download {}: {}", download_id, e);
        AppError::internal_error("Failed to delete download").to_api_error()
    })?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_download_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsDelete,)>(op)
        .id("LlmModel.deleteDownload")
        .tag("LLM Models - Downloads")
        .summary("Delete a terminal download instance")
        .description("Delete a download that is completed, failed, or cancelled. Active downloads cannot be deleted.")
        .response_with::<204, (), _>(|res| res.description("Download deleted successfully"))
        .response_with::<400, (), _>(|res| res.description("Cannot delete active download"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// GET /api/llm-models/downloads/subscribe
/// Subscribe to all active download progress updates via SSE
#[debug_handler]
pub async fn subscribe_download_progress(
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let client_id = Uuid::new_v4();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Add client to the connection pool
    {
        let mut clients = SSE_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
        clients.insert(client_id, tx.clone());
    }

    // Send initial connection event
    let connected_event = SSEDownloadProgressEvent::Connected(SSEDownloadProgressConnectedData {
        message: Some("Connected to download progress stream".to_string()),
    });

    let _ = tx.send(Ok(connected_event.into()));

    // Start monitoring if not already active
    start_download_monitoring().await;

    // Create the SSE stream with proper cleanup
    let stream = async_stream::stream! {
        // Keep the sender alive for the stream lifetime
        let _tx_keeper = tx;

        while let Some(event) = rx.recv().await {
            yield event;
        }

        // Stream ended, remove client
        tracing::info!("Download monitoring client disconnected: {}", client_id);
        remove_client(client_id);
    };

    // Keep-alive, as every other SSE route in this tree already does
    // (`chat/stream/handler.rs`, the framework's `sync/routes.rs`,
    // `hardware/handlers.rs`, voice, workflow, code_sandbox). This was the ONLY
    // SSE endpoint without it. A download stream is idle by design between
    // progress ticks — and completely silent once the monitor loop exits — so
    // without a heartbeat there is nothing on the wire to distinguish "waiting"
    // from "dead", and any intermediary on the path (the ngrok tunnel this app
    // supports, a reverse proxy) is entitled to reap it.
    Ok((StatusCode::OK, Sse::new(stream).keep_alive(KeepAlive::default())))
}

pub fn subscribe_download_progress_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsRead,)>(op)
        .id("LlmModel.subscribeDownloadProgress")
        .tag("LLM Models - Downloads")
        .summary("Subscribe to download progress via SSE")
        .description("Real-time Server-Sent Events stream of download progress. Updates every 1 second. Auto-closes when no active downloads remain.")
        .response::<200, Json<SSEDownloadProgressEvent>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// =====================================================
// SSE Helper Functions
// =====================================================

/// Start download monitoring service
async fn start_download_monitoring() {
    let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    if *monitoring_active {
        return; // Already running
    }
    *monitoring_active = true;
    drop(monitoring_active);

    tracing::info!("Starting download monitoring service");

    tokio::spawn(async move {
        // 1s update cadence. tokio's `interval` fires the first tick
        // immediately (`MissedTickBehavior::Burst` default), so this
        // also gives the user feedback within a tick of the
        // monitoring task spawning. Halved from the original 2s to
        // tighten perceived responsiveness on the first few hundred
        // MB of a fast download. Zero wire traffic when no downloads
        // are active — the loop self-terminates at line ~447 when
        // `get_all_active` returns empty, so the only cost of the
        // tighter cadence is during active downloads.
        let mut interval = interval(Duration::from_secs(1));
        let mut last_downloads_state: Option<String> = None;

        loop {
            interval.tick().await;

            // Check if we have any connected clients
            let client_count = {
                let clients = SSE_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
                clients.len()
            };

            if client_count == 0 {
                // No clients connected, stop monitoring
                tracing::info!("No clients connected, stopping download monitoring");
                let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                *monitoring_active = false;
                break;
            }

            // Fetch active downloads
            let downloads = Repos.download_instance.get_all_active().await;

            match downloads {
                Ok(downloads) => {
                    if downloads.is_empty() {
                        // No more active downloads, send complete event and stop
                        let complete_event = SSEDownloadProgressEvent::Complete(
                            "All downloads completed".to_string(),
                        );
                        broadcast_event(complete_event.into()).await;

                        tracing::info!("All downloads completed, stopping download monitoring");
                        let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                        *monitoring_active = false;
                        break;
                    } else {
                        let progress_updates: Vec<DownloadProgressUpdate> =
                            downloads.iter().map(DownloadProgressUpdate::from).collect();

                        let update_event = SSEDownloadProgressEvent::Update(progress_updates);
                        let downloads_json = update_event.data().unwrap_or_default();

                        // Only send update if state has changed
                        if last_downloads_state.as_ref() != Some(&downloads_json) {
                            last_downloads_state = Some(downloads_json.clone());
                            broadcast_event(update_event.into()).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get downloads: {}", e);
                    let error_event =
                        SSEDownloadProgressEvent::Error(format!("Failed to get downloads: {}", e));
                    broadcast_event(error_event.into()).await;

                    // Stop monitoring on error
                    let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                    *monitoring_active = false;
                    break;
                }
            }
        }
    });
}

/// Broadcast event to all connected clients
async fn broadcast_event(event: Event) {
    let clients = {
        let clients = SSE_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
        clients.clone()
    };

    if clients.is_empty() {
        return;
    }

    // Send to all clients and track disconnected ones
    let mut disconnected_clients = Vec::new();

    for (client_id, tx) in clients.iter() {
        if tx.send(Ok(event.clone())).is_err() {
            disconnected_clients.push(*client_id);
        }
    }

    // Remove disconnected clients
    if !disconnected_clients.is_empty() {
        let mut clients = SSE_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
        for client_id in disconnected_clients {
            clients.remove(&client_id);
            tracing::info!(
                "Removed disconnected download monitoring client: {}",
                client_id
            );
        }
    }
}

/// Remove client from connection pool
fn remove_client(client_id: ClientId) {
    let mut clients = SSE_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
    clients.remove(&client_id);
    tracing::info!("Removed download monitoring client: {}", client_id);
}

/// TEST-9 — pin the FLAT wire shape of `DownloadProgressUpdate`.
///
/// The reported "0 Bytes / 0 Bytes" bug was a client that merged this event into
/// a `DownloadInstance` with a spread, unaware that the server FLATTENS
/// `progress_data` into top-level fields while every UI reads `progress_data.*`.
/// The client now maps field-by-field — which is only correct while the wire
/// stays flat. This test is what makes a later nesting of the payload fail HERE,
/// next to the `From` impl, instead of silently re-breaking the progress bar.
#[cfg(test)]
mod wire_shape_tests {
    use super::*;
    use crate::modules::llm_model::models::DownloadProgressData;

    fn instance_with(progress: Option<DownloadProgressData>) -> DownloadInstance {
        DownloadInstance {
            id: Uuid::new_v4(),
            provider_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            request_data: Default::default(),
            status: DownloadStatus::Downloading,
            progress_data: progress.into(),
            error_message: None,
            started_at: chrono::Utc::now(),
            completed_at: None,
            model_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn progress_update_flattens_progress_data_to_the_top_level() {
        let download = instance_with(Some(DownloadProgressData {
            phase: DownloadPhase::Downloading,
            current: 5_637_699_037,
            total: 5_680_522_464,
            message: "Downloading model weights".to_string(),
            speed_bps: 1_606_723,
            eta_seconds: 26,
        }));

        let update = DownloadProgressUpdate::from(&download);

        // Serialize rather than read the fields: the CLIENT sees JSON, and what
        // it must map is the JSON shape, not the Rust struct.
        let json = serde_json::to_value(&update).expect("update must serialize");
        let obj = json.as_object().expect("update must be a JSON object");

        assert!(
            obj.get("progress_data").is_none(),
            "the SSE payload must stay FLAT — a nested `progress_data` would \
             silently re-break every consumer that maps the flat fields; got {json}"
        );
        assert_eq!(obj.get("current").and_then(|v| v.as_i64()), Some(5_637_699_037));
        assert_eq!(obj.get("total").and_then(|v| v.as_i64()), Some(5_680_522_464));
        assert_eq!(obj.get("speed_bps").and_then(|v| v.as_i64()), Some(1_606_723));
        assert_eq!(obj.get("eta_seconds").and_then(|v| v.as_i64()), Some(26));
        assert_eq!(
            obj.get("message").and_then(|v| v.as_str()),
            Some("Downloading model weights")
        );
    }

    /// A download with no `progress_data` yet emits the byte fields as JSON
    /// `null`, NOT as zeros. That distinction is what lets the client keep the
    /// figure already on screen instead of blanking it back to "0 Bytes".
    #[test]
    fn absent_progress_data_yields_nulls_not_zeros() {
        let update = DownloadProgressUpdate::from(&instance_with(None));
        let json = serde_json::to_value(&update).expect("update must serialize");
        for field in ["current", "total", "speed_bps", "eta_seconds", "message"] {
            assert!(
                json.get(field).is_some_and(|v| v.is_null()),
                "{field} must be null when the row has no progress_data yet, so the \
                 consumer can distinguish 'unknown' from 'zero'; got {json}"
            );
        }
        // `phase` is the ONE progress field that is NOT optional: it is filled
        // with `Created` even here. The consumer must therefore not treat its
        // presence as evidence that progress is known — a guard that did was
        // inert on every real frame (audit round 2).
        assert_eq!(
            json.get("phase").and_then(|v| v.as_str()),
            Some("created"),
            "phase is required on the wire and defaults to `created`; got {json}"
        );
    }

    /// The two WHOLE-ROW fields carry the row's current value on every frame, so
    /// an absent one serialises as an explicit `null` rather than being omitted.
    /// The consumer relies on that to distinguish "cleared" from "unknown" — it
    /// takes these as-is instead of falling back, which is only correct while
    /// the key is always present.
    #[test]
    fn whole_row_fields_are_present_as_null_when_unset() {
        let update = DownloadProgressUpdate::from(&instance_with(None));
        let json = serde_json::to_value(&update).expect("update must serialize");
        for field in ["error_message", "model_id"] {
            assert!(
                json.get(field).is_some_and(|v| v.is_null()),
                "{field} must be present as null so a server-side CLEAR is \
                 observable to the consumer; got {json}"
            );
        }
    }
}
