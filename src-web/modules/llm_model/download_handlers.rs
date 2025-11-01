// LLM Model Download Management Handlers
// Source: react-test/src-tauri/src/api/download_instances.rs
// Following ziee-chat patterns with handlers and docs together

use aide::transform::TransformOperation;
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Extension, Json,
};
use sqlx::PgPool;
use futures_util::stream::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::{
    models::{DownloadInstance, DownloadPhase, DownloadStatus},
    permissions::*,
    repository::DownloadInstanceRepository,
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
    pub status: String,
    pub phase: DownloadPhase,
    pub current: Option<i64>,
    pub total: Option<i64>,
    pub message: Option<String>,
    pub speed_bps: Option<i64>,
    pub eta_seconds: Option<i64>,
    pub error_message: Option<String>,
}

impl From<&DownloadInstance> for DownloadProgressUpdate {
    fn from(download: &DownloadInstance) -> Self {
        DownloadProgressUpdate {
            id: download.id.to_string(),
            status: download.status.as_str().to_string(),
            phase: download
                .progress_data
                .as_ref()
                .map(|p| p.phase)
                .unwrap_or(DownloadPhase::Created),
            current: download.progress_data.as_ref().map(|p| p.current),
            total: download.progress_data.as_ref().map(|p| p.total),
            message: download.progress_data.as_ref().and_then(|p| Some(p.message.clone())),
            speed_bps: download.progress_data.as_ref().map(|p| p.speed_bps),
            eta_seconds: download.progress_data.as_ref().map(|p| p.eta_seconds),
            error_message: download.error_message.clone(),
        }
    }
}

/// SSE connected event data
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEDownloadProgressConnectedData {
    pub message: Option<String>,
}

/// SSE event types for download progress
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum SSEDownloadProgressEvent {
    Connected(SSEDownloadProgressConnectedData),
    Update(Vec<DownloadProgressUpdate>),
    Complete(String),
    Error(String),
}

impl SSEDownloadProgressEvent {
    /// Convert enum to JSON data string
    pub fn data(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }
}

impl From<SSEDownloadProgressEvent> for Event {
    fn from(event: SSEDownloadProgressEvent) -> Self {
        let event_type = match &event {
            SSEDownloadProgressEvent::Connected(_) => "Connected",
            SSEDownloadProgressEvent::Update(_) => "Update",
            SSEDownloadProgressEvent::Complete(_) => "Complete",
            SSEDownloadProgressEvent::Error(_) => "Error",
        };

        let data = serde_json::to_string(&event).unwrap_or_default();

        Event::default()
            .event(event_type)
            .data(data)
    }
}

// =====================================================
// Download Management Handlers
// =====================================================

/// GET /api/llm-models/downloads
/// List all download instances (paginated, with optional status filter)
#[debug_handler]
pub async fn list_all_downloads(
    State(_pool): State<PgPool>,
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    Query(params): Query<DownloadPaginationQuery>,
    Extension(repo): Extension<DownloadInstanceRepository>,
) -> ApiResult<Json<DownloadInstanceListResponse>> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    // Parse status filter if provided
    let status_filter = params
        .status
        .as_ref()
        .and_then(|s| DownloadStatus::from_str(s));

    let response = repo.list(page, per_page, status_filter).await
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
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}

/// GET /api/llm-models/downloads/{download_id}
/// Get a specific download instance by ID
#[debug_handler]
pub async fn get_download(
    State(_pool): State<PgPool>,
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    Path(download_id): Path<Uuid>,
    Extension(repo): Extension<DownloadInstanceRepository>,
) -> ApiResult<Json<DownloadInstance>> {
    let download = repo.get_by_id(download_id).await
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
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// POST /api/llm-models/downloads/{download_id}/cancel
/// Cancel an active download
#[debug_handler]
pub async fn cancel_download(
    State(_pool): State<PgPool>,
    _auth: RequirePermissions<(LlmModelsDownloadsCancel,)>,
    Path(download_id): Path<Uuid>,
    Extension(repo): Extension<DownloadInstanceRepository>,
) -> ApiResult<StatusCode> {
    // Verify the download exists and user has access
    let download = repo.get_by_id(download_id).await
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
                "Download cannot be cancelled in its current state"
            ),
        ));
    }

    // Signal cancellation to the background download task first
    let cancellation_result = crate::utils::cancellation::cancel_download(download_id).await;

    if cancellation_result {
        tracing::info!("Download {} cancellation signal sent successfully", download_id);
    } else {
        tracing::warn!("Download {} was not being tracked for cancellation", download_id);
    }

    // Update status to cancelled first so users can see the cancellation
    let cancel_request = UpdateDownloadStatusRequest {
        status: DownloadStatus::Cancelled,
        error_message: Some("Cancelled by user".to_string()),
        model_id: None,
    };

    let _updated = repo.update_status(download_id, cancel_request).await
        .map_err(|e| {
            tracing::error!("Failed to cancel download {}: {}", download_id, e);
            AppError::internal_error("Failed to cancel download").to_api_error()
        })?
        .ok_or_else(|| AppError::not_found("Download instance").to_api_error())?;

    tracing::info!("Download {} marked as cancelled", download_id);

    // Spawn a background task to delete the cancelled download after 60 seconds
    let repo_clone = repo.clone();
    tokio::spawn(async move {
        tracing::info!("Scheduling deletion of cancelled download {} in 60 seconds", download_id);
        tokio::time::sleep(Duration::from_secs(60)).await;

        match repo_clone.delete(download_id).await {
            Ok(true) => {
                tracing::info!("Successfully deleted cancelled download {} after 60 seconds", download_id);
            }
            Ok(false) => {
                tracing::warn!("Cancelled download {} was already deleted", download_id);
            }
            Err(e) => {
                tracing::error!("Failed to delete cancelled download {} after 60 seconds: {}", download_id, e);
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
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// DELETE /api/llm-models/downloads/{download_id}
/// Delete a download instance (only terminal states)
#[debug_handler]
pub async fn delete_download(
    State(_pool): State<PgPool>,
    _auth: RequirePermissions<(LlmModelsDownloadsDelete,)>,
    Path(download_id): Path<Uuid>,
    Extension(repo): Extension<DownloadInstanceRepository>,
) -> ApiResult<StatusCode> {
    // Verify the download exists and user has access
    let download = repo.get_by_id(download_id).await
        .map_err(|e| {
            tracing::error!("Failed to verify download {}: {}", download_id, e);
            AppError::internal_error("Database operation failed").to_api_error()
        })?
        .ok_or_else(|| AppError::not_found("Download instance").to_api_error())?;

    // Only allow deleting terminal states
    if !download.is_terminal() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_STATE",
                "Cannot delete active download"
            ),
        ));
    }

    let deleted = repo.delete(download_id).await
        .map_err(|e| {
            tracing::error!("Failed to delete download {}: {}", download_id, e);
            AppError::internal_error("Failed to delete download").to_api_error()
        })?;

    if !deleted {
        return Err(AppError::not_found("Download instance").to_api_error());
    }

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
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Download not found"))
}

/// GET /api/llm-models/downloads/subscribe
/// Subscribe to all active download progress updates via SSE
#[debug_handler]
pub async fn subscribe_download_progress(
    State(_pool): State<PgPool>,
    _auth: RequirePermissions<(LlmModelsDownloadsRead,)>,
    Extension(repo): Extension<DownloadInstanceRepository>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Create interval for polling (every 2 seconds)
    let mut interval_stream = IntervalStream::new(interval(Duration::from_secs(2)));

    // Create the stream
    let stream = async_stream::stream! {
        let mut last_downloads_state: Option<String>;

        // Send initial connected event
        let connected_event = SSEDownloadProgressEvent::Connected(
            SSEDownloadProgressConnectedData {
                message: Some("Connected to download progress stream".to_string()),
            }
        );
        yield Ok(connected_event.into());

        // Send initial update immediately
        let downloads = repo.get_all_active().await;

        match downloads {
            Ok(downloads) => {
                if downloads.is_empty() {
                    // No active downloads, send complete event and close
                    let complete_event = SSEDownloadProgressEvent::Complete(
                        "No active downloads".to_string()
                    );
                    yield Ok(complete_event.into());
                    return;
                } else {
                    let progress_updates: Vec<DownloadProgressUpdate> =
                        downloads.iter().map(DownloadProgressUpdate::from).collect();

                    let update_event = SSEDownloadProgressEvent::Update(progress_updates);
                    let downloads_json = update_event.data().unwrap_or_default();

                    last_downloads_state = Some(downloads_json.clone());

                    yield Ok(update_event.into());
                }
            }
            Err(e) => {
                let error_event = SSEDownloadProgressEvent::Error(
                    format!("Failed to get downloads: {}", e)
                );
                yield Ok(error_event.into());
                return;
            }
        }

        // Poll for updates - the stream will be automatically dropped when client disconnects
        while let Some(_) = interval_stream.next().await {
            let downloads = repo.get_all_active().await;

            match downloads {
                Ok(downloads) => {
                    if downloads.is_empty() {
                        // No more active downloads, send complete event and close
                        let complete_event = SSEDownloadProgressEvent::Complete(
                            "All downloads completed".to_string()
                        );
                        yield Ok(complete_event.into());
                        break;
                    } else {
                        let progress_updates: Vec<DownloadProgressUpdate> =
                            downloads.iter().map(DownloadProgressUpdate::from).collect();

                        let update_event = SSEDownloadProgressEvent::Update(progress_updates);
                        let downloads_json = update_event.data().unwrap_or_default();

                        // Only send update if state has changed
                        if last_downloads_state.as_ref() != Some(&downloads_json) {
                            last_downloads_state = Some(downloads_json.clone());

                            yield Ok(update_event.into());
                        }
                    }
                }
                Err(e) => {
                    let error_event = SSEDownloadProgressEvent::Error(
                        format!("Failed to get downloads: {}", e)
                    );
                    yield Ok(error_event.into());
                    break;
                }
            }
        }
    };

    Ok((
        StatusCode::OK,
        Sse::new(stream).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        ),
    ))
}

pub fn subscribe_download_progress_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDownloadsRead,)>(op)
        .id("LlmModel.subscribeDownloadProgress")
        .tag("LLM Models - Downloads")
        .summary("Subscribe to download progress via SSE")
        .description("Real-time Server-Sent Events stream of download progress. Updates every 2 seconds. Auto-closes when no active downloads remain.")
        .response::<200, Json<SSEDownloadProgressEvent>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}
