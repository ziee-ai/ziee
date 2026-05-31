//! Tier 2 — proxy front-door auth + model resolution (PRE-forward).
//!
//! These cases all reject before any engine is contacted, so no engine
//! spawn is needed. The proxy is a public-shaped HTTP boundary even
//! though only the same process talks to it today.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn missing_authorization_header_is_401() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(server.api_url("/local-llm/v1/chat/completions"))
        .json(&json!({ "model": "anything", "messages": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_bearer_is_401() {
    let server = TestServer::start().await;
    let resp = lrt::proxy_chat(&server, "not-a-real-token", json!({ "model": "x", "messages": [] })).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn valid_token_unknown_model_is_404() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_pid, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    let resp = lrt::proxy_chat(&server, &proxy_token, json!({ "model": "no-such-model", "messages": [] })).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_model_field_is_400() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_pid, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    let resp = lrt::proxy_chat(&server, &proxy_token, json!({ "messages": [] })).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn non_json_body_is_400() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_pid, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/local-llm/v1/chat/completions"))
        .header("Authorization", format!("Bearer {proxy_token}"))
        .header("Content-Type", "application/json")
        .body("this is not json{{{")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// Provider A's token cannot resolve provider B's model — and the miss is
/// a 404 (not 403), so cross-provider model existence isn't leaked.
#[tokio::test]
async fn cross_provider_model_is_404() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let pool = lrt::test_pool(&server).await;

    let (_a_id, a_token, _a) = lrt::create_local_provider_with_token(&server, &admin.token).await;
    let (b_id, _b_token, _b) = lrt::create_local_provider_with_token(&server, &admin.token).await;

    // A model that exists ONLY under provider B.
    let b_model = lrt::create_local_model(&server, &admin.token, b_id, "secret-b-model", "llamacpp", None).await;
    lrt::mark_model_valid(&pool, b_model).await;

    // Provider A's token asking for B's model name → 404.
    let resp = lrt::proxy_chat(&server, &a_token, json!({ "model": "secret-b-model", "messages": [] })).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// A model whose validation_status is `failed` is refused with 503 before
/// any engine contact.
#[tokio::test]
async fn validation_failed_model_is_503() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let pool = lrt::test_pool(&server).await;

    let (pid, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;
    let model_id = lrt::create_local_model(&server, &admin.token, pid, "broken-model", "llamacpp", None).await;
    sqlx::query("UPDATE llm_models SET validation_status = 'failed' WHERE id = $1")
        .bind(model_id)
        .execute(&pool)
        .await
        .unwrap();

    let resp = lrt::proxy_chat(&server, &proxy_token, json!({ "model": "broken-model", "messages": [] })).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}
