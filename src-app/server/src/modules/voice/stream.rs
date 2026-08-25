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
    // cap is intentionally NOT enforced here (see the module docstring); the
    // per-tick COST bound is `stream_max_decode_secs` applied below.
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

    // Cost bound: clamp the interim clip to its trailing window before decoding, so
    // per-tick whisper cost is bounded regardless of recording length (FB-1). Clips
    // at/under the window are decoded whole (fully stitched); longer ones show the
    // recent window. The FINAL on-stop decode (batch /transcribe) is unclamped.
    let audio =
        axum::body::Bytes::from(clamp_wav_tail(&audio, settings.stream_max_decode_secs.max(1) as u32));

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

/// Clamp a 16 kHz mono 16-bit WAV to its trailing `secs` seconds of PCM — the
/// per-tick interim cost bound (FB-1). Rewrites the RIFF/`data` chunk sizes and
/// re-emits a canonical 44-byte-header WAV. Best-effort: returns the input
/// unchanged when the header can't be parsed OR the clip is already at/under the
/// window (so short dictation stays fully stitched). Pure → unit-testable.
pub(super) fn clamp_wav_tail(wav: &[u8], secs: u32) -> Vec<u8> {
    if secs == 0 || wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return wav.to_vec();
    }
    // Walk chunks after the 12-byte RIFF header to find `fmt ` + `data`.
    let mut pos = 12usize;
    let (mut sample_rate, mut channels, mut bits) = (0u32, 0u16, 0u16);
    let (mut data_off, mut data_len) = (0usize, 0usize);
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let size =
            u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= wav.len() {
            channels = u16::from_le_bytes([wav[body + 2], wav[body + 3]]);
            sample_rate =
                u32::from_le_bytes([wav[body + 4], wav[body + 5], wav[body + 6], wav[body + 7]]);
            bits = u16::from_le_bytes([wav[body + 14], wav[body + 15]]);
        } else if id == b"data" {
            data_off = body;
            data_len = size.min(wav.len().saturating_sub(body));
            break;
        }
        pos = body + size + (size & 1); // chunks are word-aligned
    }
    // fmt fields are user-controlled (validate_wav only checks the RIFF/WAVE
    // magic), so compute the frame/byte rates with overflow-safe arithmetic —
    // a crafted fmt (e.g. sample_rate=1e9, channels=2, bits=32) must not panic
    // under overflow-checks. Any implausible/overflowing header → no-op (leave
    // whisper the whole clip; it will reject a genuinely malformed WAV itself).
    let block_align = ((bits as u32 / 8) * channels as u32).max(1); // ≤ 8191*65535, no overflow
    let byte_rate = match sample_rate.checked_mul(block_align) {
        Some(br) if br > 0 && data_off != 0 => br,
        _ => return wav.to_vec(),
    };
    let want = (byte_rate as usize).saturating_mul(secs as usize);
    if data_len <= want {
        return wav.to_vec(); // already within the window → no clamp (full stitch)
    }
    // Keep the LAST `want` bytes of PCM, frame-aligned.
    let mut start = data_len - want;
    start -= start % block_align as usize;
    let tail = &wav[data_off + start..data_off + data_len];

    let mut out = Vec::with_capacity(44 + tail.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + tail.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&(block_align as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(tail.len() as u32).to_le_bytes());
    out.extend_from_slice(tail);
    out
}

#[cfg(test)]
mod tests {
    use super::clamp_wav_tail;

    /// Build a 16 kHz mono 16-bit WAV of `secs` seconds (ramp samples, non-zero).
    fn make_wav(secs: f64) -> Vec<u8> {
        let (sr, ch, bits) = (16_000u32, 1u16, 16u16);
        let byte_rate = sr * ch as u32 * (bits / 8) as u32;
        let data_len = (byte_rate as f64 * secs) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&ch.to_le_bytes());
        w.extend_from_slice(&sr.to_le_bytes());
        w.extend_from_slice(&byte_rate.to_le_bytes());
        w.extend_from_slice(&(ch * bits / 8).to_le_bytes());
        w.extend_from_slice(&bits.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..data_len {
            w.push((i & 0xff) as u8);
        }
        w
    }

    fn data_secs(wav: &[u8]) -> f64 {
        // fmt byte_rate at offset 28; data size at offset 40.
        let byte_rate =
            u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]) as f64;
        let data_len = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as f64;
        data_len / byte_rate
    }

    #[test]
    fn clamps_a_long_clip_to_the_trailing_window() {
        let wav = make_wav(10.0);
        let clamped = clamp_wav_tail(&wav, 3);
        assert!(&clamped[0..4] == b"RIFF" && &clamped[8..12] == b"WAVE", "valid WAV");
        let secs = data_secs(&clamped);
        assert!((secs - 3.0).abs() < 0.05, "expected ~3s, got {secs}");
        // The kept bytes are the TAIL of the original data (last sample preserved).
        assert_eq!(*clamped.last().unwrap(), *wav.last().unwrap(), "keeps the newest audio");
    }

    #[test]
    fn is_a_noop_when_clip_is_within_the_window() {
        let wav = make_wav(2.0);
        // 2s clip, 30s window → returned unchanged (fully stitched).
        assert_eq!(clamp_wav_tail(&wav, 30), wav);
        // Exactly at the window boundary → still unchanged.
        assert_eq!(clamp_wav_tail(&wav, 2), wav);
    }

    #[test]
    fn is_a_noop_on_non_wav_or_zero_secs() {
        assert_eq!(clamp_wav_tail(b"not a wav", 3), b"not a wav".to_vec());
        assert_eq!(clamp_wav_tail(&[], 3), Vec::<u8>::new());
        let wav = make_wav(10.0);
        assert_eq!(clamp_wav_tail(&wav, 0), wav, "secs=0 → no clamp");
    }

    /// A crafted `fmt ` with an overflowing byte_rate (sample_rate 1e9, 2ch, 32-bit)
    /// must NOT panic (overflow-checks in dev/test) — it falls back to the whole clip.
    #[test]
    fn does_not_panic_on_overflowing_fmt() {
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&1000u32.to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&2u16.to_le_bytes()); // channels
        w.extend_from_slice(&1_000_000_000u32.to_le_bytes()); // sample_rate
        w.extend_from_slice(&0u32.to_le_bytes()); // byte_rate (ignored — recomputed)
        w.extend_from_slice(&8u16.to_le_bytes()); // block_align
        w.extend_from_slice(&32u16.to_le_bytes()); // bits → sample_rate*block_align overflows u32
        w.extend_from_slice(b"data");
        w.extend_from_slice(&64u32.to_le_bytes());
        w.extend(std::iter::repeat(1u8).take(64));
        // Must return the input unchanged rather than panicking.
        assert_eq!(clamp_wav_tail(&w, 3), w);
    }
}
