//! Tier 2 — local provider create flow: one-time token + server-derived
//! base_url (NULL in DB, derived on read).

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use uuid::Uuid;

#[tokio::test]
async fn local_create_returns_one_time_token() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_pid, token, provider) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    assert_eq!(provider["provider_type"].as_str(), Some("local"));
    // 32-byte url-safe base64 (no pad) == 43 chars.
    assert_eq!(token.len(), 43, "proxy token should be a 43-char url-safe string");
}

/// base_url is NOT stored; it's derived from the server's listen config +
/// LOCAL_PROXY_PATH on every read, and forced to loopback.
#[tokio::test]
async fn base_url_is_derived_on_read() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (provider_id, _token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    // Read the provider back (a read path → the derivation seam runs).
    let resp = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let provider = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str() == Some(provider_id.to_string().as_str()))
        .expect("provider present in list");

    let expected = format!("{}/api/local-llm/v1", server.base_url);
    assert_eq!(
        provider["base_url"].as_str(),
        Some(expected.as_str()),
        "base_url should be the live-derived loopback proxy URL"
    );
}

/// A remote provider create does NOT return a plaintext token.
#[tokio::test]
async fn remote_create_has_no_plaintext_token() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({
            "name": format!("remote-{}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "openai",
            "api_key": "sk-test",
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["plaintext_api_key"].is_null(),
        "remote provider create must not surface a plaintext token"
    );
}
