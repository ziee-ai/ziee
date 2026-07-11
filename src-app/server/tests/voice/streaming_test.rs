//! TEST-3..6 — the `POST /api/voice/transcribe/stream` interim (live-caption)
//! endpoint.
//!
//! TEST-3 (hero) runs the REAL production auto-start path against the freshly-built
//! `stub-whisper-server`: it forwards the FULL accumulating buffer to `/inference`
//! and returns the current transcript, and proves the clip-length cap is NOT
//! enforced on the interim path (an in-progress buffer legitimately grows past it).
//! TEST-4 proves the independent `streaming_enabled` 409 gate (batch still works).
//! TEST-5 is the auth/permission deny path (A9). TEST-6 proves caps + magic-byte
//! sniff reject bad uploads with a clean 4xx (never 500) before any runtime runs.

use serde_json::{Value, json};

use super::{insert_version_row, make_wav, stage_model, stub_whisper_binary, VOICE_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

/// Multipart-post a WAV to the streaming endpoint, optionally with a token.
async fn post_stream(server: &TestServer, token: Option<&str>, wav: Vec<u8>) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    let mut req = reqwest::Client::new()
        .post(server.api_url("/voice/transcribe/stream"))
        .multipart(form);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    req.send().await.expect("stream transcribe request")
}

/// Register the stub whisper runtime + stage the default `base` model so the
/// production auto-start path can spawn + forward for real.
async fn provision_stub_runtime(server: &TestServer) {
    let stub = stub_whisper_binary();
    insert_version_row(server, "v0.0.0-stub", "cpu", stub.to_string_lossy().as_ref(), true).await;
    stage_model(server, "base");
}

async fn put_settings(server: &TestServer, admin_token: &str, body: Value) {
    let r = reqwest::Client::new()
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "settings PUT should 200");
}

/// TEST-3 — the hero. A REAL stub-whisper-server is auto-started and the canned
/// transcript comes back through the full interim path; the clip-length cap is
/// NOT enforced (a 3s clip passes even with max_clip_seconds=1).
#[tokio::test]
async fn test_stream_returns_transcript_and_ignores_clip_len() {
    let server = TestServer::start().await;
    provision_stub_runtime(&server).await;
    let admin = create_user_with_permissions(&server, "voice_stream_admin", VOICE_ADMIN_PERMS).await;
    // A cap that batch would reject — the interim path must ignore it.
    put_settings(&server, &admin.token, json!({ "max_clip_seconds": 1 })).await;

    let user = create_user_with_permissions(&server, "voice_stream_user", &[]).await;
    let resp = post_stream(&server, Some(&user.token), make_wav(3.0)).await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "stream should 200 despite 3s > max_clip_seconds=1 (body: {body})");

    let parsed: Value = serde_json::from_str(&body).unwrap();
    let text = parsed["text"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        text.contains("quick brown fox"),
        "interim transcript should contain the stub's canned phrase; got: {:?}",
        parsed["text"]
    );
    assert!(parsed["language"].is_string(), "language field present");
    assert!(parsed["duration_ms"].is_number(), "duration_ms field present");
}

/// TEST-4 — `streaming_enabled=false` blocks the interim path with 409 while the
/// batch path keeps working (the two modes toggle independently).
#[tokio::test]
async fn test_stream_409_when_streaming_disabled_batch_still_works() {
    let server = TestServer::start().await;
    provision_stub_runtime(&server).await;
    let admin = create_user_with_permissions(&server, "voice_toggle_admin", VOICE_ADMIN_PERMS).await;
    // Master `enabled` stays true; only live captions are turned off.
    put_settings(&server, &admin.token, json!({ "streaming_enabled": false })).await;

    let user = create_user_with_permissions(&server, "voice_toggle_user", &[]).await;

    // Interim → 409 (live captions disabled).
    let resp = post_stream(&server, Some(&user.token), make_wav(1.0)).await;
    assert_eq!(resp.status(), 409, "streaming_enabled=false → 409 on the interim path");

    // Batch → still 200 (unaffected by the streaming toggle).
    let part = reqwest::multipart::Part::bytes(make_wav(1.0))
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let resp = reqwest::Client::new()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "batch transcribe unaffected by streaming toggle");
}

/// TEST-5 — auth/permission deny path (A9): 401 without a token, 403 for a user
/// lacking `voice::transcribe`.
#[tokio::test]
async fn test_stream_requires_permission() {
    let server = TestServer::start().await;

    let resp = post_stream(&server, None, make_wav(0.5)).await;
    assert_eq!(resp.status(), 401, "no token → 401");

    let noperm = create_user_with_no_permissions(&server, "voice_stream_noperm").await;
    let resp = post_stream(&server, Some(&noperm.token), make_wav(0.5)).await;
    assert_eq!(resp.status(), 403, "missing voice::transcribe → 403");
}

/// TEST-6 — caps + magic-byte sniff. Over-`max_upload_bytes`, a non-WAV body, and a
/// missing `file` field each fail with a clean 4xx and NEVER a 500, before any
/// whisper runtime is touched.
#[tokio::test]
async fn test_stream_caps_and_garbage_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_streamcap_admin", VOICE_ADMIN_PERMS).await;
    let user = create_user_with_permissions(&server, "voice_streamcap_user", &[]).await;

    // Over max_upload_bytes: shrink the byte cap, send a larger WAV.
    put_settings(&server, &admin.token, json!({ "max_upload_bytes": 2048 })).await;
    let resp = post_stream(&server, Some(&user.token), make_wav(1.0)).await; // ~32 KB > 2048
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "over-bytes → clean 4xx, got {st}");
    assert_eq!(st, 400, "over-max_upload_bytes returns 400");

    // Non-WAV garbage body (under the byte cap): magic-byte sniff rejects with 400.
    put_settings(&server, &admin.token, json!({ "max_upload_bytes": 33_554_432i64 })).await;
    let garbage = vec![0x42u8; 4096];
    let resp = post_stream(&server, Some(&user.token), garbage).await;
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "non-WAV → clean 4xx (not 500), got {st}");
    assert_eq!(st, 400, "non-WAV returns 400");

    // Missing `file` field → clean 4xx (not 500).
    let empty = reqwest::multipart::Form::new().text("notfile", "x");
    let resp = reqwest::Client::new()
        .post(server.api_url("/voice/transcribe/stream"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(empty)
        .send()
        .await
        .unwrap();
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "no-file → clean 4xx, got {st}");
}
