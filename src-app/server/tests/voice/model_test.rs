//! TEST-19 — the ggml model status + download admin surface.
//!
//! The happy path is asserted by PRE-STAGING the model file (the air-gap path)
//! and observing `GET /model/status` flip `present:false → true` — this avoids
//! needing to satisfy the pinned-sha256 table for network bytes (a mock can't
//! produce a file whose digest matches the real HF pin; the sha256 verify path
//! itself is covered by the `model.rs` unit tests). A negative test drives the
//! REAL download path against a `WHISPER_MODEL_MIRROR` loopback that serves
//! mismatched bytes, proving the endpoint is reachable and FAILS CLOSED on a
//! digest mismatch (no green-washing — the external HTTP boundary is the only
//! thing mocked).

use serde_json::Value;

use super::{stage_model, VOICE_ADMIN_PERMS};
use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;

async fn get_model_status(server: &TestServer, token: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url("/voice/model/status"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("model status request")
}

#[tokio::test]
async fn test_model_status_present_flips_around_prestage() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_model_admin", VOICE_ADMIN_PERMS).await;

    // Absent initially.
    let res = get_model_status(&server, &admin.token).await;
    assert_eq!(res.status(), 200);
    let st: Value = res.json().await.unwrap();
    assert_eq!(st["model"], "base", "reports the configured model");
    assert_eq!(st["present"], false, "no model staged yet");
    assert!(st["size_bytes"].is_null(), "no size when absent");

    // Pre-stage the model file (air-gap path).
    let path = stage_model(&server, "base");
    let expected_len = std::fs::metadata(&path).unwrap().len() as i64;

    // Now present, with the on-disk size.
    let res = get_model_status(&server, &admin.token).await;
    let st: Value = res.json().await.unwrap();
    assert_eq!(st["present"], true, "staged model is detected");
    assert_eq!(st["size_bytes"], expected_len, "reports the on-disk size");
}

#[tokio::test]
async fn test_model_status_forbidden_for_non_admin() {
    let server = TestServer::start().await;
    // Default Users member: voice::transcribe but no voice::admin::read.
    let user = create_user_with_permissions(&server, "voice_model_plain", &[]).await;
    let res = get_model_status(&server, &user.token).await;
    assert_eq!(res.status(), 403, "model status needs voice::admin::read");
}

/// Negative real-path test: the model download FAILS CLOSED when the fetched
/// bytes don't match the pinned sha256 (the model host is a loopback mock via
/// the debug-only `WHISPER_MODEL_MIRROR` seam). Proves the download endpoint is
/// wired to the real verify path, not silently trusting network bytes.
#[tokio::test]
async fn test_model_download_rejects_sha256_mismatch() {
    // Loopback "HF" mirror serving arbitrary (wrong) bytes for ggml-base.bin.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = axum::Router::new().route(
        "/{*path}",
        axum::routing::get(|| async { "these bytes are not the real whisper model" }),
    );
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    let mirror = format!("http://127.0.0.1:{port}");

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WHISPER_MODEL_MIRROR".to_string(), mirror)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "voice_dl_model_admin", VOICE_ADMIN_PERMS).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/voice/model/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "model": "base" }))
        .send()
        .await
        .unwrap();
    assert!(
        !res.status().is_success(),
        "download of sha-mismatched bytes must FAIL, got {}",
        res.status()
    );

    // The mismatched partial must NOT be published — status stays absent.
    let st: Value = get_model_status(&server, &admin.token).await.json().await.unwrap();
    assert_eq!(st["present"], false, "a rejected download leaves no model on disk");

    handle.abort();
}
