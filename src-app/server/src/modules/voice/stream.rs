//! `POST /api/voice/transcribe/stream` — the interim (live-caption) endpoint.
//!
//! The composer calls this repeatedly while recording, each time posting the WHOLE
//! accumulating 16 kHz mono WAV; the handler forwards the full buffer to the same
//! managed whisper-server `/inference` the batch path uses and returns the current
//! full transcript, which the client renders as a live caption. Because whisper
//! re-decodes the whole clip each call, the caption is a coherent *stitched*
//! transcript for free (no server-side windowing / stitching).
//!
//! It differs from the batch `transcribe` handler in exactly three ways:
//!   1. It additionally requires `settings.streaming_enabled` (else 409) — the
//!      deployment live-captions toggle, independent of the master `enabled`.
//!   2. It does NOT enforce `max_clip_seconds` — an interim buffer legitimately
//!      grows toward the cap; the client stops recording at the cap, and the final
//!      authoritative decode goes through the batch endpoint which DOES enforce it.
//!   3. It forwards with a shorter interim timeout so a slow tick can't wedge the
//!      client's single-flight loop.
//!
//! It never sends a chat message; the transcript is a preview for the client.

use std::time::{Duration, Instant};

use aide::transform::TransformOperation;
use axum::extract::Multipart;
use axum::{Json, http::StatusCode};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::auto_start::{self, InflightGuard};
use super::models::TranscriptionResponse;
use super::permissions::VoiceTranscribe;
use super::transcribe::{forward_to_whisper, read_audio_field, validate_wav};

/// Interim decode ceiling. A single interim decode must stay responsive so the
/// client's next tick isn't blocked; the batch final decode keeps the 300s ceiling.
const INTERIM_WHISPER_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn transcribe_stream(
    _auth: RequirePermissions<(VoiceTranscribe,)>,
    mut multipart: Multipart,
) -> ApiResult<Json<TranscriptionResponse>> {
    let settings = Repos.voice.get_settings().await?;
    if !settings.enabled {
        return Err(AppError::conflict("voice dictation is disabled").into());
    }
    // The live-captions availability toggle (distinct from the master enable).
    if !settings.streaming_enabled {
        return Err(AppError::conflict("live voice captions are disabled").into());
    }

    let audio = read_audio_field(&mut multipart).await?;

    // Logical size cap (bytes) from settings — same as batch. NOTE: the clip-length
    // cap is intentionally NOT enforced here (see the module docstring).
    if audio.len() as i64 > settings.max_upload_bytes {
        return Err(AppError::bad_request(
            "VOICE_CLIP_TOO_LARGE",
            format!(
                "audio is {} bytes, exceeds the configured cap of {} bytes",
                audio.len(),
                settings.max_upload_bytes
            ),
        )
        .into());
    }

    validate_wav(&audio)?;

    // Ensure the managed whisper-server is up (lazy auto-start), shared with batch.
    let _guard = InflightGuard::acquire();
    let handle = auto_start::ensure_running().await?;
    auto_start::touch_last_used();

    let lang = if settings.language.trim().is_empty() {
        "auto".to_string()
    } else {
        settings.language.clone()
    };

    let started = Instant::now();
    // Interim decode is best-effort. A slow/failed tick — e.g. a long buffer whose
    // full-clip decode exceeds INTERIM_WHISPER_TIMEOUT — is TRANSIENT: the client
    // single-flights and simply skips this caption, so surface a transient 503
    // (`VOICE_INTERIM_UNAVAILABLE`) rather than a 500 that reads as a real fault.
    let text = match forward_to_whisper(&handle.base_url, audio, &lang, INTERIM_WHISPER_TIMEOUT).await
    {
        Ok(text) => text,
        Err(e) => {
            tracing::debug!("voice interim decode transient failure: {e}");
            return Err(AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "VOICE_INTERIM_UNAVAILABLE",
                "interim transcription is temporarily unavailable",
            )
            .into());
        }
    };
    let duration_ms = started.elapsed().as_millis() as i64;

    Ok((
        StatusCode::OK,
        Json(TranscriptionResponse {
            text,
            language: lang,
            duration_ms,
        }),
    ))
}

pub fn transcribe_stream_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceTranscribe,)>(op)
        .id("Voice.transcribeStream")
        .tag("Voice")
        .summary("Interim (live-caption) transcription of an in-progress recording")
        .response::<200, Json<TranscriptionResponse>>()
}
