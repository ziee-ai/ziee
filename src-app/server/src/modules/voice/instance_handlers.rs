//! Admin REST handlers for the whisper-server instance + ggml model.
//!
//! GET  `/voice/instance`         → instance snapshot (VoiceAdminRead)
//! POST `/voice/instance/restart` → drain + restart with the configured model (VoiceAdminManage)
//! POST `/voice/instance/stop`    → SIGTERM the instance (VoiceAdminManage)
//! GET  `/voice/model/status`     → configured model presence + size (VoiceAdminRead)
//! POST `/voice/model/download`   → ensure the configured (or requested) model is on disk (VoiceAdminManage)
//!
//! These are admin-only, so the instance's loopback `base_url` is returned
//! unredacted (only holders of `voice::admin::read` can see it).

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};
use aide::transform::TransformOperation;
use axum::{Json, http::StatusCode};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::auto_start;
use super::permissions::{VoiceAdminManage, VoiceAdminRead};

// ───────────────────────────── DTOs ─────────────────────────────

/// Snapshot of the single managed whisper-server instance (singleton row).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceInstanceInfo {
    /// Configured/active whisper model file (e.g. `ggml-base.bin`), if any.
    pub active_model: Option<String>,
    /// Loopback port the whisper-server is bound to, if running.
    pub local_port: Option<i32>,
    /// Loopback base URL (admin-only surface, so returned unredacted).
    pub base_url: Option<String>,
    /// Coarse lifecycle: `stopped` | `running`.
    pub status: String,
    /// Fine health-state-machine name (starting/healthy/unhealthy/…/failed).
    pub state: String,
    pub restart_attempts: i32,
    pub last_failure_reason: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub state_changed_at: DateTime<Utc>,
    /// Live OS pid of the running whisper-server (F10), if any.
    pub pid: Option<i32>,
    /// Seconds since the running process started (F10), if any.
    pub uptime_seconds: Option<i64>,
}

/// Readiness of the configured (or a requested) ggml model on disk.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceModelStatus {
    pub model: String,
    pub present: bool,
    pub size_bytes: Option<i64>,
}

// ───────────────────────────── instance ─────────────────────────────

async fn read_instance() -> Result<VoiceInstanceInfo, AppError> {
    let row = sqlx::query!(
        r#"SELECT active_model, local_port, base_url, status, state,
                  restart_attempts, last_failure_reason,
                  last_used_at as "last_used_at: DateTime<Utc>",
                  state_changed_at as "state_changed_at: DateTime<Utc>"
           FROM voice_runtime_instance WHERE id = TRUE"#,
    )
    .fetch_one(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    // Enrich with live process pid/uptime from the deployment layer (F10).
    let live = super::deployment::get_deployment_manager().local().status().await;
    Ok(VoiceInstanceInfo {
        active_model: row.active_model,
        local_port: row.local_port,
        base_url: row.base_url,
        status: row.status,
        state: row.state,
        restart_attempts: row.restart_attempts,
        last_failure_reason: row.last_failure_reason,
        last_used_at: row.last_used_at,
        state_changed_at: row.state_changed_at,
        pid: live.pid,
        uptime_seconds: live.uptime_seconds,
    })
}

pub async fn get_instance(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<VoiceInstanceInfo>> {
    Ok((StatusCode::OK, Json(read_instance().await?)))
}

pub fn get_instance_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getInstance")
        .tag("Voice")
        .summary("Read the managed whisper-server instance state")
        .response::<200, Json<VoiceInstanceInfo>>()
}

pub async fn restart_instance(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
) -> ApiResult<Json<VoiceInstanceInfo>> {
    auto_start::admin_restart().await?;
    Ok((StatusCode::OK, Json(read_instance().await?)))
}

pub fn restart_instance_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.restartInstance")
        .tag("Voice")
        .summary("Restart the whisper-server with the configured model")
        .response::<200, Json<VoiceInstanceInfo>>()
}

pub async fn stop_instance(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
) -> ApiResult<Json<VoiceInstanceInfo>> {
    auto_start::admin_stop().await?;
    Ok((StatusCode::OK, Json(read_instance().await?)))
}

pub fn stop_instance_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.stopInstance")
        .tag("Voice")
        .summary("Stop the managed whisper-server instance")
        .response::<200, Json<VoiceInstanceInfo>>()
}

// ───────────────────────────── model ─────────────────────────────

pub async fn get_model_status(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<VoiceModelStatus>> {
    let model = Repos.voice.get_settings().await?.model;
    let present = super::model::model_present(&model);
    let size_bytes = if present {
        std::fs::metadata(super::model::model_path(&model))
            .ok()
            .map(|m| m.len() as i64)
    } else {
        None
    };
    Ok((
        StatusCode::OK,
        Json(VoiceModelStatus {
            model,
            present,
            size_bytes,
        }),
    ))
}

pub fn get_model_status_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getModelStatus")
        .tag("Voice")
        .summary("Whisper ggml model presence + size on disk")
        .response::<200, Json<VoiceModelStatus>>()
}

// ───────────────────────────── logs ─────────────────────────────

/// Recent captured whisper-server stdout/stderr (ring buffer). F8/ITEM-30.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceLogsResponse {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct LogsQuery {
    /// How many trailing lines to return (default 200, capped at 1000).
    #[serde(default)]
    pub lines: Option<usize>,
}

pub async fn get_logs(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
    axum::extract::Query(q): axum::extract::Query<LogsQuery>,
) -> ApiResult<Json<VoiceLogsResponse>> {
    let n = q.lines.unwrap_or(200).min(1000);
    let lines = super::deployment::get_deployment_manager().local().logs(n).await;
    Ok((StatusCode::OK, Json(VoiceLogsResponse { lines })))
}

pub fn get_logs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getInstanceLogs")
        .tag("Voice")
        .summary("Recent whisper-server log lines")
        .response::<200, Json<VoiceLogsResponse>>()
}

/// SSE tail of whisper-server logs (snapshot + live lines).
pub async fn stream_logs(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<
    axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>>,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};
    let subscribed = super::deployment::get_deployment_manager()
        .local()
        .subscribe_logs()
        .await;
    let stream = async_stream::stream! {
        let Some((mut rx, snapshot)) = subscribed else {
            // Nothing running — emit nothing and close.
            return;
        };
        for line in snapshot {
            yield Ok::<Event, axum::Error>(Event::default().data(line));
        }
        loop {
            match rx.recv().await {
                Ok(line) => yield Ok(Event::default().data(line)),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };
    Ok((StatusCode::OK, Sse::new(stream).keep_alive(KeepAlive::default())))
}

pub fn stream_logs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.streamInstanceLogs")
        .tag("Voice")
        .summary("SSE stream of whisper-server logs")
}

// ───────────────────────────── router ─────────────────────────────

/// Admin instance + model routes. Merged into the voice router by the routes
/// layer (`voice/routes.rs`).
pub fn voice_instance_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/voice/instance",
            get_with(get_instance, get_instance_docs),
        )
        .api_route(
            "/voice/instance/restart",
            post_with(restart_instance, restart_instance_docs),
        )
        .api_route(
            "/voice/instance/stop",
            post_with(stop_instance, stop_instance_docs),
        )
        .api_route(
            "/voice/instance/logs",
            get_with(get_logs, get_logs_docs),
        )
        .api_route(
            "/voice/instance/logs/stream",
            get_with(stream_logs, stream_logs_docs),
        )
        .api_route(
            "/voice/model/status",
            get_with(get_model_status, get_model_status_docs),
        )
}
