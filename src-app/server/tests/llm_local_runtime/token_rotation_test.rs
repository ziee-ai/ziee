//! Tier 2 — proxy-token rotation. After rotation the old token 401s and
//! the new token authenticates (proven via a 404 model-miss, which only
//! happens AFTER auth succeeds).

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

async fn rotate(server: &TestServer, token: &str, provider_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}/rotate-proxy-token")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn rotate_invalidates_old_token_and_activates_new() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (provider_id, t1, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    // T1 authenticates: 404 (model-miss) means auth passed.
    let r = lrt::proxy_chat(&server, &t1, json!({ "model": "nope", "messages": [] })).await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND, "T1 should authenticate");

    // Rotate.
    let resp = rotate(&server, &admin.token, provider_id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let t2 = body["plaintext_api_key"].as_str().expect("new token").to_string();
    assert_ne!(t1, t2, "rotation must mint a different token");

    // Old token now rejected.
    let old = lrt::proxy_chat(&server, &t1, json!({ "model": "nope", "messages": [] })).await;
    assert_eq!(old.status(), StatusCode::UNAUTHORIZED, "T1 should be revoked");

    // New token authenticates.
    let new = lrt::proxy_chat(&server, &t2, json!({ "model": "nope", "messages": [] })).await;
    assert_eq!(new.status(), StatusCode::NOT_FOUND, "T2 should authenticate");
}

#[tokio::test]
async fn rotate_requires_edit_permission() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (provider_id, _t1, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    let reader = create_user_with_only_permissions(&server, "reader", &["llm_providers::read"]).await;
    let resp = rotate(&server, &reader.token, provider_id).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "rotate needs llm_providers::edit");
}
