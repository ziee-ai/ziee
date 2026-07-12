//! `POST /api/voice/transcribe` — the one user-facing endpoint.
//!
//! Accepts a browser-recorded 16 kHz mono WAV (multipart field `file`), enforces
//! the settings caps, ensures the managed whisper-server is running, forwards the
//! audio to its `/inference` endpoint, and returns the transcript. The transcript
//! is inserted into the composer for review by the client — this endpoint never
//! sends a chat message.

use std::time::Instant;

use aide::transform::TransformOperation;
use axum::extract::Multipart;
use axum::{Json, http::StatusCode};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::auto_start::{self, InflightGuard};
use super::models::TranscriptionResponse;
use super::permissions::VoiceTranscribe;

/// Max bytes we will buffer from the multipart field before the logical cap
/// check — a hard ceiling above the per-route `DefaultBodyLimit`.
const ABSOLUTE_MAX_BYTES: usize = 256 * 1024 * 1024;

pub async fn transcribe(
    _auth: RequirePermissions<(VoiceTranscribe,)>,
    mut multipart: Multipart,
) -> ApiResult<Json<TranscriptionResponse>> {
    let settings = Repos.voice.get_settings().await?;
    if !settings.enabled {
        return Err(AppError::conflict("voice dictation is disabled").into());
    }

    // Pull the `file` field bytes. Keep the `Bytes` (refcounted) rather than
    // copying into a `Vec` — it is forwarded to whisper-server as-is below.
    let audio = read_audio_field(&mut multipart).await?;

    // Logical size cap (bytes) from settings.
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

    // Validate it is a WAV and enforce the clip-length cap (best-effort from the
    // header; a non-parseable header falls back to the byte cap already applied).
    validate_wav(&audio)?;
    if let Some(secs) = wav_duration_secs(&audio)
        && secs > settings.max_clip_seconds as f64
    {
        return Err(AppError::bad_request(
            "VOICE_CLIP_TOO_LONG",
            format!(
                "clip is {:.1}s, exceeds the configured cap of {}s",
                secs, settings.max_clip_seconds
            ),
        )
        .into());
    }

    // Ensure the managed whisper-server is up (lazy auto-start). A failure here
    // maps to a clear 409/503, never a 500.
    let _guard = InflightGuard::acquire();
    let handle = auto_start::ensure_running().await?;
    auto_start::touch_last_used();

    let lang = if settings.language.trim().is_empty() {
        "auto".to_string()
    } else {
        settings.language.clone()
    };

    let started = Instant::now();
    // Batch transcription keeps the generous 300s ceiling (whisper cold-start +
    // a full 120s clip decode). The streaming handler passes a shorter interim
    // timeout so a slow tick can't wedge its loop.
    let text = forward_to_whisper(
        &handle.base_url,
        audio,
        &lang,
        std::time::Duration::from_secs(300),
    )
    .await?;
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

pub fn transcribe_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceTranscribe,)>(op)
        .id("Voice.transcribe")
        .tag("Voice")
        .summary("Transcribe a recorded audio clip (WAV) into text for the composer")
        .response::<200, Json<TranscriptionResponse>>()
}

/// Read the single multipart `file` field (the WAV) as refcounted `Bytes`,
/// enforcing the absolute hard ceiling. Shared by the batch + streaming handlers.
/// A transport/parse error is a clean 400 (`VOICE_BAD_UPLOAD`); a missing field is
/// `VOICE_NO_AUDIO`; over-ceiling is `VOICE_CLIP_TOO_LARGE`.
pub(super) async fn read_audio_field(
    multipart: &mut Multipart,
) -> Result<axum::body::Bytes, AppError> {
    let mut audio: Option<axum::body::Bytes> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return Err(AppError::bad_request(
                    "VOICE_BAD_UPLOAD",
                    format!("malformed multipart upload: {e}"),
                ));
            }
        };
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request("VOICE_BAD_UPLOAD", format!("read field: {e}")))?;
            if bytes.len() > ABSOLUTE_MAX_BYTES {
                return Err(AppError::bad_request(
                    "VOICE_CLIP_TOO_LARGE",
                    "audio upload exceeds the maximum size",
                ));
            }
            audio = Some(bytes);
        }
    }
    audio.ok_or_else(|| {
        AppError::bad_request("VOICE_NO_AUDIO", "missing multipart `file` field (audio/wav)")
    })
}

/// The SHARED, pooled loopback client for whisper-server `/inference` forwards
/// (F5/ITEM-28): built once with `.no_proxy()` (so an env HTTP proxy can't reroute
/// the 127.0.0.1 call) and reused across requests (a per-request timeout is set on
/// the request builder, not the client).
fn inference_client() -> &'static reqwest::Client {
    static INFERENCE_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    INFERENCE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("build whisper inference client")
    })
}

/// POST the WAV to whisper-server's native `/inference` endpoint (multipart) and
/// parse the `{ "text": ... }` response. Shared by the batch transcribe handler
/// (300s) and the streaming handler (a shorter interim `timeout`).
pub(super) async fn forward_to_whisper(
    base_url: &str,
    audio: axum::body::Bytes,
    lang: &str,
    timeout: std::time::Duration,
) -> Result<String, AppError> {
    // `Part::stream_with_length` over the refcounted `Bytes` avoids the extra
    // full-clip copy `Part::bytes` (which needs an owned `Vec`) would force,
    // while still sending a known Content-Length (not chunked).
    let len = audio.len() as u64;
    let part = reqwest::multipart::Part::stream_with_length(reqwest::Body::from(audio), len)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(AppError::internal_with_id)?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("response_format", "json");
    // whisper-server treats an empty/`auto` language as auto-detect.
    if lang != "auto" {
        form = form.text("language", lang.to_string());
    }

    // Loopback forward to whisper-server via a SHARED keep-alive client
    // (F5/ITEM-28); the per-request timeout is applied on the request builder below.
    let client = inference_client();

    let resp = client
        .post(format!("{}/inference", base_url.trim_end_matches('/')))
        .timeout(timeout)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("whisper inference request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "whisper-server /inference returned HTTP {}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| AppError::internal_error(format!("read inference response: {e}")))?;
    parse_inference_text(&body)
}

/// Extract the transcript from a whisper-server `/inference` JSON response body.
fn parse_inference_text(body: &str) -> Result<String, AppError> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| AppError::internal_error(format!("parse inference JSON: {e}")))?;
    // whisper-server returns `{ "text": "..." }`; some builds nest under
    // `transcription`. Accept either.
    let text = v
        .get("text")
        .or_else(|| v.get("transcription"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(text)
}

/// Validate the bytes are a RIFF/WAVE container. Returns a 4xx (not 500) on a
/// non-WAV body so a bad client upload is a clear client error. Shared with the
/// streaming handler.
pub(super) fn validate_wav(bytes: &[u8]) -> Result<(), AppError> {
    let is_wav = bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE";
    if !is_wav {
        return Err(AppError::bad_request(
            "VOICE_NOT_WAV",
            "audio must be a 16 kHz mono WAV (RIFF/WAVE)",
        )
        .into());
    }
    Ok(())
}

/// Best-effort WAV duration (seconds) from the `fmt `/`data` chunks. `None` when
/// the header can't be parsed (caller falls back to the byte cap).
fn wav_duration_secs(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    // Walk chunks after the 12-byte RIFF header.
    let mut pos = 12usize;
    let mut byte_rate: Option<u32> = None;
    let mut data_len: Option<u32> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]])
            as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            byte_rate = Some(u32::from_le_bytes([
                bytes[body + 8],
                bytes[body + 9],
                bytes[body + 10],
                bytes[body + 11],
            ]));
        } else if id == b"data" {
            data_len = Some(size as u32);
            break;
        }
        pos = body + size + (size & 1); // chunks are word-aligned
    }
    match (byte_rate, data_len) {
        (Some(br), Some(dl)) if br > 0 => Some(dl as f64 / br as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-31: the inference-forward client is a SINGLE shared instance (pooled,
    // not built per-request) — two calls return the same `&'static` client. It's
    // constructed with `.no_proxy()` (F5, verified at the construction site).
    #[test]
    fn inference_client_is_a_single_shared_instance() {
        assert!(
            std::ptr::eq(inference_client(), inference_client()),
            "the inference client must be a single shared instance, not per-request"
        );
    }

    /// Build a minimal 16 kHz mono 16-bit WAV of `secs` seconds of silence.
    fn make_wav(secs: f64) -> Vec<u8> {
        let sample_rate = 16_000u32;
        let channels = 1u16;
        let bits = 16u16;
        let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
        let data_len = (byte_rate as f64 * secs) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&channels.to_le_bytes());
        w.extend_from_slice(&sample_rate.to_le_bytes());
        w.extend_from_slice(&byte_rate.to_le_bytes());
        w.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        w.extend_from_slice(&bits.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        w.extend(std::iter::repeat(0u8).take(data_len as usize));
        w
    }

    #[test]
    fn accepts_valid_wav_rejects_garbage() {
        assert!(validate_wav(&make_wav(1.0)).is_ok());
        assert!(validate_wav(b"not a wav at all").is_err());
        assert!(validate_wav(&[]).is_err());
    }

    #[test]
    fn computes_wav_duration() {
        let d = wav_duration_secs(&make_wav(3.0)).expect("duration");
        assert!((d - 3.0).abs() < 0.05, "expected ~3s, got {d}");
    }

    #[test]
    fn parses_inference_text_variants() {
        assert_eq!(parse_inference_text(r#"{"text":"  hi there "}"#).unwrap(), "hi there");
        assert_eq!(
            parse_inference_text(r#"{"transcription":"nested"}"#).unwrap(),
            "nested"
        );
        assert_eq!(parse_inference_text(r#"{"other":1}"#).unwrap(), "");
    }
}
