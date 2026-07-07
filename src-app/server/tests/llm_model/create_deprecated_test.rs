//! TEST-9 (ITEM-9): creating a model whose id the curated catalog marks
//! `deprecated` persists `is_deprecated = true` at create time — the periodic
//! sweep is the backstop for models that get deprecated later.

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

async fn create_openai_provider(server: &TestServer, token: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": format!("openai-{}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "openai",
            "base_url": "https://api.openai.com/v1",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

async fn create_model(
    server: &TestServer,
    token: &str,
    provider_id: &str,
    name: &str,
) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": name,
            "display_name": name,
            "engine_type": "mistralrs",
            "file_format": "safetensors",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "model create should 201");
    resp.json().await.unwrap()
}

#[tokio::test]
async fn create_flags_catalog_deprecated_model() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "dep_admin",
        &["llm_providers::create", "llm_models::create", "llm_models::read"],
    )
    .await;

    let provider_id = create_openai_provider(&server, &admin.token).await;

    // gpt-3.5-turbo is `deprecated: true` in ai-providers/data/known_models.json.
    let deprecated = create_model(&server, &admin.token, &provider_id, "gpt-3.5-turbo").await;
    assert_eq!(
        deprecated["is_deprecated"].as_bool(),
        Some(true),
        "a catalog-deprecated model must be flagged at create time"
    );

    // gpt-4o is a current model — not deprecated.
    let current = create_model(&server, &admin.token, &provider_id, "gpt-4o").await;
    assert_eq!(
        current["is_deprecated"].as_bool(),
        Some(false),
        "a current catalog model must not be flagged"
    );

    // The flag must also be persisted (re-read via GET).
    let id = deprecated["id"].as_str().unwrap();
    let got = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), StatusCode::OK);
    let got_body: serde_json::Value = got.json().await.unwrap();
    assert_eq!(got_body["is_deprecated"].as_bool(), Some(true));
}
