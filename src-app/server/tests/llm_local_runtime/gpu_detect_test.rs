//! Tier 2 — GPU detection endpoint shape + permission gating.

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_no_permissions};
use super::test_helpers::LOCAL_RUNTIME_ADMIN_PERMS;
use reqwest::StatusCode;

#[tokio::test]
async fn detect_gpu_returns_expected_shape() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/local-runtime/detect-gpu"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert!(body["available"].is_array(), "available backends array");
    assert!(body["recommended"].is_string(), "recommended backend");
    assert!(body["platform"].is_string(), "platform");
    assert!(body["arch"].is_string(), "arch");
    // CPU is always an available fallback.
    let available = body["available"].as_array().unwrap();
    assert!(
        available.iter().any(|b| b.as_str() == Some("cpu")),
        "cpu should always be available: {body}"
    );
}

#[tokio::test]
async fn detect_gpu_requires_read_permission() {
    let server = TestServer::start().await;
    let user = create_user_with_no_permissions(&server, "noperm").await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/local-runtime/detect-gpu"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
