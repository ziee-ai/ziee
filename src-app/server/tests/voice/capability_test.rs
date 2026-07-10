//! TEST-33 — `GET /api/voice/capability` (the composer mic readiness snapshot).
//!
//! Reachable by a normal `voice::transcribe` user (NOT admin-gated), reports
//! `can_transcribe=false` with the right sub-flags when unprovisioned and `true`
//! once a runtime row + model file exist. A user lacking `voice::transcribe`
//! gets 403.

use serde_json::Value;

use super::{insert_version_row, stage_model, stub_whisper_binary};
use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

async fn get_capability(server: &TestServer, token: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url("/voice/capability"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("capability request")
}

#[tokio::test]
async fn test_capability_reflects_provisioning_state() {
    let server = TestServer::start().await;
    // A normal user — capability must be reachable WITHOUT admin perms.
    let user = create_user_with_permissions(&server, "voice_cap_user", &[]).await;

    // Unprovisioned: enabled (settings default) but no runtime + no model.
    let resp = get_capability(&server, &user.token).await;
    assert_eq!(resp.status(), 200, "capability reachable by a transcribe user");
    let cap: Value = resp.json().await.unwrap();
    assert_eq!(cap["enabled"], true, "runtime settings default enabled");
    assert_eq!(cap["runtime_ready"], false, "no runtime installed yet");
    assert_eq!(cap["model_ready"], false, "no model staged yet");
    assert_eq!(cap["can_transcribe"], false, "unprovisioned → not usable");
    assert_eq!(cap["model"], "base", "default model surfaced");

    // Provision a runtime row (host platform/arch) + stage the model file.
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

    // Now the mic is usable.
    let resp = get_capability(&server, &user.token).await;
    assert_eq!(resp.status(), 200);
    let cap: Value = resp.json().await.unwrap();
    assert_eq!(cap["runtime_ready"], true, "runtime row present → ready");
    assert_eq!(cap["model_ready"], true, "model staged → ready");
    assert_eq!(cap["can_transcribe"], true, "enabled + runtime + model → usable");
}

#[tokio::test]
async fn test_capability_forbidden_without_transcribe_permission() {
    let server = TestServer::start().await;
    let user = create_user_with_no_permissions(&server, "voice_cap_noperm").await;
    let resp = get_capability(&server, &user.token).await;
    assert_eq!(resp.status(), 403, "capability needs voice::transcribe → 403");
}
