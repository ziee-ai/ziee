//! Tier 2 — validation-by-loading.
//!
//! Local validation spawns the engine; with the stub it reaches a
//! terminal state and extracts capabilities from the GGUF header. (The
//! stub can't *fail* on bad model bytes since it ignores `--model`, so
//! the "corrupt file → validation_warning" case lives in the gold-smoke
//! test against a real engine.) Remote validation is an inline probe,
//! exercised here via wiremock.

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::mock_release;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn model_validation_status(server: &TestServer, token: &str, model_id: Uuid) -> String {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/{model_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    body["validation_status"].as_str().unwrap_or("").to_string()
}

#[tokio::test]
async fn local_validate_returns_202_queued() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (pid, _t, _p) = lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    let model_id = lrt::create_local_model(&mock.server, &admin.token, pid, "v-queue", "llamacpp", None).await;
    lrt::seed_model_file(&pool, model_id, "/tmp/ziee-validate-queue.gguf").await;

    let resp = reqwest::Client::new()
        .post(mock.server.api_url(&format!("/llm-models/{model_id}/validate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "local validate queues (202)");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["queued"].as_bool(), Some(true));
}

/// A valid GGUF on disk → validation reaches a terminal state and the
/// engine load probe (stub /health) passes → `valid`.
#[tokio::test]
async fn local_validate_reaches_terminal_state() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (pid, _t, _p) = lrt::create_local_provider_with_token(&mock.server, &admin.token).await;

    // Write a real, valid tiny GGUF to disk so capability extraction can
    // parse it; seed the file row at that path.
    let gguf_path = std::env::temp_dir().join(format!("ziee-validate-{}.gguf", Uuid::new_v4()));
    std::fs::write(&gguf_path, lrt::tiny_gguf_bytes()).unwrap();
    let model_id = lrt::create_local_model(&mock.server, &admin.token, pid, "v-real", "llamacpp", None).await;
    lrt::seed_model_file(&pool, model_id, &gguf_path.to_string_lossy()).await;

    // Trigger.
    let resp = reqwest::Client::new()
        .post(mock.server.api_url(&format!("/llm-models/{model_id}/validate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Poll until validation leaves the pending/processing states.
    let mut status = String::new();
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(750)).await;
        status = model_validation_status(&mock.server, &admin.token, model_id).await;
        if matches!(status.as_str(), "valid" | "validation_warning" | "invalid" | "failed") {
            break;
        }
    }
    assert!(
        matches!(status.as_str(), "valid" | "validation_warning"),
        "stub load probe should leave the model usable, got '{status}'"
    );

    let _ = std::fs::remove_file(&gguf_path);
}

#[tokio::test]
async fn validate_requires_edit_permission() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (pid, _t, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;
    let model_id = lrt::create_local_model(&server, &admin.token, pid, "v-perm", "llamacpp", None).await;

    let reader = create_user_with_only_permissions(&server, "reader", &["llm_models::read"]).await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/{model_id}/validate")))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "validate needs llm_models::edit");
}

/// Remote provider validation is an inline probe: a provider whose
/// upstream returns 401 validates as not-valid (200 response, valid=false).
#[tokio::test]
async fn remote_validate_reports_invalid_on_401() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    // Upstream that rejects everything with 401.
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "bad key", "type": "authentication_error"}
        })))
        .mount(&upstream)
        .await;

    // Remote provider pointing at the mock upstream.
    let create = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("remote-{}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "openai",
            "api_key": "bad-key",
            "base_url": format!("{}/v1", upstream.uri()),
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let pbody: serde_json::Value = create.json().await.unwrap();
    // Provider fields are flattened to the top level (serde flatten).
    let provider_id = Uuid::parse_str(pbody["id"].as_str().unwrap()).unwrap();

    let model_id = lrt::create_local_model(&server, &admin.token, provider_id, "gpt-x", "llamacpp", None).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/{model_id}/validate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    // Remote probe runs inline (not queued) and returns 200 with valid=false.
    assert_eq!(resp.status(), StatusCode::OK, "remote validate runs inline");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"].as_bool(), Some(false), "401 upstream → not valid");
}
