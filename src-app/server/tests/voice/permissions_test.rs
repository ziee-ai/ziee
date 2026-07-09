//! TEST-13 — permission gating on the voice surface.
//!
//! transcribe: 401 without a token, 403 for a user lacking `voice::transcribe`,
//! reachable (NOT 401/403) for a default Users member (migration 134 grants the
//! perm). Admin endpoints: 403 for a non-admin.

use super::make_wav;
use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

/// Multipart-post a WAV, optionally with a bearer token.
async fn post_transcribe(server: &TestServer, token: Option<&str>, wav: Vec<u8>) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    let mut req = reqwest::Client::new()
        .post(server.api_url("/voice/transcribe"))
        .multipart(form);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    req.send().await.expect("transcribe request")
}

#[tokio::test]
async fn test_transcribe_requires_authentication() {
    let server = TestServer::start().await;
    let resp = post_transcribe(&server, None, make_wav(0.5)).await;
    assert_eq!(resp.status(), 401, "no token → 401");
}

#[tokio::test]
async fn test_transcribe_forbidden_without_permission() {
    let server = TestServer::start().await;
    // A user stripped of ALL groups — no `voice::transcribe`.
    let user = create_user_with_no_permissions(&server, "voice_noperm").await;
    let resp = post_transcribe(&server, Some(&user.token), make_wav(0.5)).await;
    assert_eq!(resp.status(), 403, "missing voice::transcribe → 403");
}

#[tokio::test]
async fn test_transcribe_reachable_for_default_users_member() {
    let server = TestServer::start().await;
    // A default Users member inherits `voice::transcribe` (migration 134).
    let user = create_user_with_permissions(&server, "voice_member", &[]).await;
    let resp = post_transcribe(&server, Some(&user.token), make_wav(0.5)).await;
    let st = resp.status();
    // No runtime is provisioned, so this fails downstream (404/503) — but the
    // permission gate itself must PASS (never 401/403), which is the assertion.
    assert_ne!(st, 401, "authenticated member must not 401");
    assert_ne!(st, 403, "a Users member holds voice::transcribe (got 403)");
}

#[tokio::test]
async fn test_admin_endpoint_forbidden_for_non_admin() {
    let server = TestServer::start().await;
    // Default Users member: holds voice::transcribe but NOT voice::admin::read.
    let user = create_user_with_permissions(&server, "voice_plain", &[]).await;
    let resp = reqwest::Client::new()
        .get(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "settings read needs voice::admin::read → 403");
}
