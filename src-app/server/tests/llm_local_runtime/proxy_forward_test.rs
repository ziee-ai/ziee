//! Tier 2 — real proxy forward (spawns the stub-engine).
//!
//! These exercise the FULL production path: proxy auth → model resolve →
//! single-flight auto-start (real spawn) → `/health` wait → bearer rewrite
//! → forward → response/SSE stream-back. The per-instance bearer lives in
//! a process-global map populated only at spawn, so a 200 round-trip
//! proves the proxy rewrote the inbound proxy-token to the engine bearer
//! (the stub 401s on a wrong bearer).

use crate::common::test_helpers::create_user_with_permissions;
use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;

/// Stand up a mock-served engine (set as default) + a startable llamacpp
/// model, returning `(proxy_token, model_name)`. The engine is NOT started
/// — the first proxy call auto-starts it.
async fn ready_model(mock: &MockReleaseServer, admin_token: &str, gguf_path: &str) -> (String, String) {
    lrt::download_engine_from_mock(mock, admin_token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, proxy_token, _p) =
        lrt::create_local_provider_with_token(&mock.server, admin_token).await;
    let name = "stub-llama";
    let version_id = {
        // The version is the system default now; pass it through for
        // required_runtime_version_id bookkeeping.
        let resp = reqwest::Client::new()
            .get(mock.server.api_url("/local-runtime/versions?engine=llamacpp"))
            .header("Authorization", format!("Bearer {admin_token}"))
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        uuid::Uuid::parse_str(body["versions"][0]["id"].as_str().unwrap()).unwrap()
    };
    lrt::make_startable_model(&mock.server, admin_token, &pool, provider_id, name, version_id, gguf_path).await;
    (proxy_token, name.to_string())
}

#[tokio::test]
async fn chat_completions_non_stream_roundtrip() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (proxy_token, model) = ready_model(&mock, &admin.token, "/tmp/ziee-stub-ok.gguf").await;

    let resp = lrt::proxy_chat(
        &mock.server,
        &proxy_token,
        json!({ "model": model, "messages": [{"role":"user","content":"hi"}], "stream": false }),
    )
    .await;
    let status = resp.status();
    let text = resp.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "auto-start + forward should 200; body: {text}");
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["model"].as_str(), Some(model.as_str()), "model echoed");
    assert_eq!(
        body["choices"][0]["message"]["content"].as_str(),
        Some("Hello from stub")
    );
}

#[tokio::test]
async fn chat_completions_sse_roundtrip() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (proxy_token, model) = ready_model(&mock, &admin.token, "/tmp/ziee-stub-sse.gguf").await;

    let resp = lrt::proxy_chat(
        &mock.server,
        &proxy_token,
        json!({ "model": model, "messages": [{"role":"user","content":"hi"}], "stream": true }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let text = resp.text().await.unwrap();
    assert!(text.contains("Hello"), "SSE body should carry chunks: {text}");
    assert!(text.contains("[DONE]"), "SSE body should terminate with [DONE]: {text}");
}

#[tokio::test]
async fn embeddings_roundtrip() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (proxy_token, model) = ready_model(&mock, &admin.token, "/tmp/ziee-stub-emb.gguf").await;

    let resp = lrt::proxy_embeddings(
        &mock.server,
        &proxy_token,
        json!({ "model": model, "input": "hello" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"][0]["embedding"].is_array(), "embedding vector returned");
}

/// Upstream non-2xx is passed through (stub forced to 500).
#[tokio::test]
async fn upstream_status_is_passed_through() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (proxy_token, model) = ready_model(&mock, &admin.token, "/tmp/ziee-stub-500.gguf").await;

    let resp = lrt::proxy_chat(
        &mock.server,
        &proxy_token,
        json!({ "model": model, "messages": [], "stub_force_status": 500 }),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "upstream 500 should pass through as 500"
    );
}

/// Engine never becomes healthy (path sentinel) + a short auto-start
/// timeout → 504.
#[tokio::test]
async fn auto_start_timeout_is_504() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    // Short timeout so the test doesn't wait 30s.
    let r = lrt::update_runtime_settings(&mock.server, &admin.token, json!({ "auto_start_timeout_secs": 2 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    // The `stub-unhealthy` sentinel in the model path makes /health 503
    // forever, so the health-wait times out.
    let (proxy_token, model) =
        ready_model(&mock, &admin.token, "/tmp/ziee-stub-unhealthy.gguf").await;

    let resp = lrt::proxy_chat(
        &mock.server,
        &proxy_token,
        json!({ "model": model, "messages": [] }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);
}

/// No engine registered at all → start fails → 502 (not a timeout).
#[tokio::test]
async fn spawn_failure_when_no_engine_is_502() {
    let server = crate::common::TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let pool = lrt::test_pool(&server).await;

    let (provider_id, proxy_token, _p) =
        lrt::create_local_provider_with_token(&server, &admin.token).await;
    let model_id =
        lrt::create_local_model(&server, &admin.token, provider_id, "no-engine-model", "llamacpp", None).await;
    lrt::seed_model_file(&pool, model_id, "/tmp/ziee-no-engine.gguf").await;
    lrt::mark_model_valid(&pool, model_id).await;

    let resp = lrt::proxy_chat(&server, &proxy_token, json!({ "model": "no-engine-model", "messages": [] })).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_GATEWAY,
        "no runtime version available → engine_start_failed → 502"
    );
}
