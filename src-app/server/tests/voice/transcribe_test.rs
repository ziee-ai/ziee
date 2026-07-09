//! TEST-11 (hero) + TEST-12 — the `POST /api/voice/transcribe` end-to-end path.
//!
//! TEST-11 runs the REAL production auto-start path: it registers a
//! `voice_runtime_versions` row pointing at the freshly-built
//! `stub-whisper-server`, pre-stages the ggml model on disk, and posts a fixture
//! WAV. The server spawns the stub via the hardened deployment path, polls its
//! `/` health, forwards the audio to `/inference`, and returns the transcript —
//! exercising spawn → health → forward → parse for real (only the transcript is
//! canned).
//!
//! TEST-12 asserts the caps + magic-byte sniff reject bad uploads with a clean
//! 4xx (never a 500) BEFORE any runtime is touched.

use serde_json::{Value, json};

use super::{insert_version_row, make_wav, stage_model, stub_whisper_binary, VOICE_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// Multipart-post a WAV to the transcribe endpoint as `token`.
async fn post_transcribe(server: &TestServer, token: &str, wav: Vec<u8>) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    reqwest::Client::new()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("transcribe request")
}

/// TEST-11 — the hero test. A REAL stub-whisper-server is auto-started and the
/// canned transcript comes back through the full production path.
#[tokio::test]
async fn test_transcribe_real_stub_whisper_returns_transcript() {
    let server = TestServer::start().await;

    // Register the stub as the system-default whisper runtime + pre-stage the
    // (default `base`) model so the air-gap path skips any download.
    let stub = stub_whisper_binary();
    insert_version_row(
        &server,
        "v0.0.0-stub",
        "cpu",
        stub.to_string_lossy().as_ref(),
        true,
    )
    .await;
    stage_model(&server, "base");

    // A plain default-Users member holds `voice::transcribe` (migration 134).
    let user = create_user_with_permissions(&server, "voice_user", &[]).await;

    let resp = post_transcribe(&server, &user.token, make_wav(1.0)).await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "transcribe should 200 (body: {body})");

    let parsed: Value = serde_json::from_str(&body).unwrap();
    let text = parsed["text"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        text.contains("quick brown fox"),
        "transcript should contain the stub's canned phrase; got: {:?}",
        parsed["text"]
    );
    // The response also echoes the language the request used.
    assert!(parsed["language"].is_string(), "language field present");
    assert!(parsed["duration_ms"].is_number(), "duration_ms field present");
}

/// TEST-12 — caps + magic-byte sniff. Over-`max_upload_bytes`,
/// over-`max_clip_seconds`, and a non-WAV body each fail with a clean 4xx and
/// NEVER a 500. These reject before any whisper runtime is spawned.
#[tokio::test]
async fn test_transcribe_caps_and_garbage_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_cap_admin", VOICE_ADMIN_PERMS).await;
    let user = create_user_with_permissions(&server, "voice_cap_user", &[]).await;
    let client = reqwest::Client::new();

    let put_settings = |body: Value| {
        let admin_token = admin.token.clone();
        let client = client.clone();
        let url = server.api_url("/voice/settings");
        async move {
            let r = client
                .put(&url)
                .header("Authorization", format!("Bearer {admin_token}"))
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), 200, "settings PUT should 200");
        }
    };

    // (a) Over max_upload_bytes: shrink the byte cap, send a larger WAV.
    put_settings(json!({ "max_upload_bytes": 2048 })).await;
    let resp = post_transcribe(&server, &user.token, make_wav(1.0)).await; // ~32 KB > 2048
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "over-bytes → clean 4xx, got {st}");
    assert_eq!(st, 400, "over-max_upload_bytes returns 400");

    // (b) Over max_clip_seconds: raise the byte cap back, cap the clip length.
    put_settings(json!({ "max_upload_bytes": 33_554_432, "max_clip_seconds": 1 })).await;
    let resp = post_transcribe(&server, &user.token, make_wav(3.0)).await; // 3s > 1s cap
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "over-clip → clean 4xx, got {st}");
    assert_eq!(st, 400, "over-max_clip_seconds returns 400");

    // (c) Non-WAV garbage body: magic-byte sniff rejects with a 4xx (not 500).
    let garbage = vec![0x42u8; 4096]; // not RIFF/WAVE, under the byte cap
    let resp = post_transcribe(&server, &user.token, garbage).await;
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "non-WAV → clean 4xx (not 500), got {st}");
    assert_eq!(st, 400, "non-WAV returns 400");

    // A missing `file` field is also a clean 4xx (not a 500).
    let empty = reqwest::multipart::Form::new().text("notfile", "x");
    let resp = client
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(empty)
        .send()
        .await
        .unwrap();
    let st = resp.status();
    assert!(st.is_client_error() && st != 500, "no-file → clean 4xx, got {st}");
}
