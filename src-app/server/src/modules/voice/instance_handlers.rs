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
}

/// Readiness of the configured (or a requested) ggml model on disk.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceModelStatus {
    pub model: String,
    pub present: bool,
    pub size_bytes: Option<i64>,
}

/// Optional body for the model download endpoint. Absent `model` → the
/// currently-configured settings model.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct DownloadModelRequest {
    #[serde(default)]
    pub model: Option<String>,
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

async fn resolve_model_name(explicit: Option<String>) -> Result<String, AppError> {
    match explicit {
        Some(m) => {
            if !super::model::is_supported_model(&m) {
                return Err(AppError::bad_request(
                    "VALIDATION_ERROR",
                    "unsupported model (expected one of: tiny, base, base.en, small)",
                ));
            }
            Ok(m)
        }
        None => Ok(Repos.voice.get_settings().await?.model),
    }
}

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

pub async fn download_model(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    body: Option<Json<DownloadModelRequest>>,
) -> ApiResult<Json<VoiceModelStatus>> {
    let requested = body.and_then(|b| b.0.model);
    let model = resolve_model_name(requested).await?;

    // Trigger the streaming, sha256-verified download (idempotent when present).
    // Uses the progress-reporting entry point (logging progress); an SSE variant
    // of this endpoint can swap the callback for an event sink.
    let model_for_log = model.clone();
    let path = super::model::download_model_with_progress(&model, move |done, total| {
        if let Some(total) = total
            && total > 0
        {
            tracing::debug!(
                "voice: model {model_for_log} download {done}/{total} ({}%)",
                done.saturating_mul(100) / total
            );
        }
    })
    .await?;
    let size_bytes = std::fs::metadata(&path).ok().map(|m| m.len() as i64);

    Ok((
        StatusCode::OK,
        Json(VoiceModelStatus {
            model,
            present: true,
            size_bytes,
        }),
    ))
}

pub fn download_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.downloadModel")
        .tag("Voice")
        .summary("Download the configured (or a specified) whisper ggml model")
        .response::<200, Json<VoiceModelStatus>>()
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
            "/voice/model/status",
            get_with(get_model_status, get_model_status_docs),
        )
        .api_route(
            "/voice/model/download",
            post_with(download_model, download_model_docs),
        )
}
