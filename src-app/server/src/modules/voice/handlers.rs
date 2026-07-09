//! HTTP handlers for the voice dictation admin + capability REST surface.
//!
//! The transcribe handler and the runtime-version / model / instance admin
//! handlers are added in the engine + lifecycle layers; this file holds the
//! settings singleton (admin) and the capability snapshot (any transcribe user).

use aide::transform::TransformOperation;
use axum::{Json, http::StatusCode};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::models::{UpdateVoiceSettingsRequest, VoiceCapability, VoiceSettings};
use super::permissions::{VoiceAdminManage, VoiceAdminRead, VoiceTranscribe};

// ───────────────────────────── settings (admin) ─────────────────────────────

pub async fn get_settings(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<VoiceSettings>> {
    let row = Repos.voice.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getSettings")
        .tag("Voice")
        .summary("Read voice dictation settings")
        .response::<200, Json<VoiceSettings>>()
}

pub async fn update_settings(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateVoiceSettingsRequest>,
) -> ApiResult<Json<VoiceSettings>> {
    // Range validation (defense-in-depth alongside the DB CHECKs → clearer errors).
    if let Some(ref m) = body.model
        && !super::model::is_supported_model(m)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "unsupported model (expected one of: tiny, base, base.en, small)",
        )
        .into());
    }
    if let Some(ref lang) = body.language {
        // Accept `auto` (whisper auto-detect) or a 2-letter ISO 639-1 code, so a
        // bad value fails with a clean 400 here instead of an opaque 503 on the
        // next transcribe. Empty is tolerated (treated as `auto` downstream).
        let l = lang.trim();
        let ok = l.is_empty()
            || l.eq_ignore_ascii_case("auto")
            || (l.len() == 2 && l.bytes().all(|b| b.is_ascii_alphabetic()));
        if !ok {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "language must be 'auto' or a 2-letter ISO 639-1 code (e.g. en, es, zh)",
            )
            .into());
        }
    }
    if let Some(n) = body.idle_unload_secs
        && !(0..=86_400).contains(&n)
    {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "idle_unload_secs out of range (0..=86400)")
                .into(),
        );
    }
    if let Some(n) = body.auto_start_timeout_secs
        && !(1..=600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "auto_start_timeout_secs out of range (1..=600)",
        )
        .into());
    }
    if let Some(n) = body.drain_timeout_secs
        && !(1..=600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "drain_timeout_secs out of range (1..=600)",
        )
        .into());
    }
    if let Some(n) = body.max_clip_seconds
        && !(1..=3_600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "max_clip_seconds out of range (1..=3600)",
        )
        .into());
    }
    if let Some(n) = body.max_upload_bytes
        && !(1_024..=67_108_864).contains(&n)
    {
        // Ceiling matches VOICE_TRANSCRIBE_BODY_LIMIT (64 MiB) so a larger
        // setting can't yield a 413 before the handler's logical cap runs.
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "max_upload_bytes out of range (1024..=67108864)",
        )
        .into());
    }

    let row = Repos
        .voice
        .update_settings(
            body.enabled,
            body.model,
            body.language,
            body.idle_unload_secs,
            body.auto_start_timeout_secs,
            body.drain_timeout_secs,
            body.max_clip_seconds,
            body.max_upload_bytes,
        )
        .await?;

    sync_publish(
        SyncEntity::VoiceSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.updateSettings")
        .tag("Voice")
        .summary("Update voice dictation settings (enable, model, language, caps)")
        .response::<200, Json<VoiceSettings>>()
}

// ─────────────────────────── capability (any user) ──────────────────────────

pub async fn get_capability(
    _auth: RequirePermissions<(VoiceTranscribe,)>,
) -> ApiResult<Json<VoiceCapability>> {
    let settings = Repos.voice.get_settings().await?;
    let runtime_ready = super::binary_manager::runtime_ready().await;
    let model_ready = super::model::model_present(&settings.model);
    let enabled = settings.enabled;
    let cap = VoiceCapability {
        enabled,
        runtime_ready,
        model_ready,
        model: settings.model,
        max_clip_seconds: settings.max_clip_seconds,
        can_transcribe: enabled && runtime_ready && model_ready,
    };
    Ok((StatusCode::OK, Json(cap)))
}

pub fn get_capability_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceTranscribe,)>(op)
        .id("Voice.capability")
        .tag("Voice")
        .summary("Voice dictation readiness for the composer mic button")
        .response::<200, Json<VoiceCapability>>()
}

// ─────────────────────── sync-cache (admin convenience) ──────────────────────

use super::runtime_version::models::SyncCacheResponse;

pub async fn sync_cache(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
) -> ApiResult<Json<SyncCacheResponse>> {
    let synced_count = super::binary_manager::sync_cache().await?;
    Ok((
        StatusCode::OK,
        Json(SyncCacheResponse {
            synced_count,
            message: format!("Synced {synced_count} cached whisper binary(ies)"),
        }),
    ))
}

pub fn sync_cache_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.syncVersionCache")
        .tag("Voice")
        .summary("Back-fill the runtime-version registry from cached whisper binaries on disk")
        .response::<200, Json<SyncCacheResponse>>()
}
