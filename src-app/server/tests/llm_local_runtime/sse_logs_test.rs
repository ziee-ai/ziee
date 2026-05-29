//! Tier 2 — engine log surfaces: the `/logs` snapshot and the
//! `/logs/stream` SSE tail (spawns the stub-engine, which prints a boot
//! line + per-request lines that flow into both surfaces).

use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use std::time::Duration;
use tokio_stream::StreamExt;
use uuid::Uuid;

async fn started(mock: &MockReleaseServer, admin_token: &str, name: &str) -> Uuid {
    let version_id = lrt::download_engine_from_mock(mock, admin_token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _t, _p) = lrt::create_local_provider_with_token(&mock.server, admin_token).await;
    let model_id = lrt::make_startable_model(
        &mock.server, admin_token, &pool, provider_id, name, version_id, "/tmp/ziee-logs.gguf",
    )
    .await;
    let start = lrt::start_instance(&mock.server, admin_token, model_id).await;
    assert_eq!(start.status(), StatusCode::CREATED);
    model_id
}

#[tokio::test]
async fn logs_snapshot_contains_boot_line() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let model_id = started(&mock, &admin.token, "logs-model").await;

    // Give the engine a moment to emit its boot line into the capture loop.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/models/{model_id}/logs")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let logs = body["logs"].as_array().expect("logs array");
    assert!(
        logs.iter().any(|l| l.as_str().unwrap_or("").contains("stub-engine")),
        "snapshot should contain the engine boot line: {body}"
    );
}

#[tokio::test]
async fn logs_stream_replays_buffer() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let model_id = started(&mock, &admin.token, "stream-model").await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/models/{model_id}/logs/stream")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("text/event-stream"), "SSE content-type, got {ct}");

    // Read chunks until we see the replayed boot line or time out.
    let mut stream = resp.bytes_stream();
    let mut acc = String::new();
    let found = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(chunk)) = stream.next().await {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("stub-engine") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(found, "SSE stream should replay the buffered boot line; got: {acc}");
}

#[tokio::test]
async fn logs_require_logs_permission() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let model_id = started(&mock, &admin.token, "gate-model").await;

    // Has instance-read but NOT the logs permission.
    let user =
        create_user_with_only_permissions(&mock.server, "reader", &["llm_local_runtime::read"]).await;
    let resp = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/models/{model_id}/logs")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "logs need the logs permission");
}
