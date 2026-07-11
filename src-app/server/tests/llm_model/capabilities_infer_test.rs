//! TEST-17 (ITEM-9): the per-model parameter-contract overrides
//! (`supports_sampling_params` / `supports_thinking` / `thinking_style`) persist
//! through the create + read path on the `capabilities` JSONB (no migration), so
//! the editable DB model row is a durable source of truth for the adapter.

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
    resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn param_contract_overrides_round_trip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "caps_admin",
        &["llm_providers::create", "llm_models::create", "llm_models::read"],
    )
    .await;
    let provider_id = create_openai_provider(&server, &admin.token).await;
    let client = reqwest::Client::new();

    // Create a model with the param-contract overrides explicitly set.
    let created = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "custom-restricted-model",
            "display_name": "Custom Restricted",
            "engine_type": "mistralrs",
            "file_format": "safetensors",
            "capabilities": {
                "chat": true,
                "supports_sampling_params": false,
                "supports_thinking": true,
                "thinking_style": "adaptive"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: serde_json::Value = created.json().await.unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["capabilities"]["supports_sampling_params"], json!(false));
    assert_eq!(created["capabilities"]["supports_thinking"], json!(true));
    assert_eq!(created["capabilities"]["thinking_style"], json!("adaptive"));

    // Re-read: the overrides persist on the JSONB.
    let got = client
        .get(server.api_url(&format!("/llm-models/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), StatusCode::OK);
    let got: serde_json::Value = got.json().await.unwrap();
    assert_eq!(got["capabilities"]["supports_sampling_params"], json!(false));
    assert_eq!(got["capabilities"]["supports_thinking"], json!(true));
    assert_eq!(got["capabilities"]["thinking_style"], json!("adaptive"));

    // A model created WITHOUT the overrides leaves them unset (the adapter then
    // resolves dynamically from the catalog + family policy at request time).
    let plain = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "plain-model",
            "display_name": "Plain",
            "engine_type": "mistralrs",
            "file_format": "safetensors",
            "capabilities": { "chat": true }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(plain.status(), StatusCode::CREATED);
    let plain: serde_json::Value = plain.json().await.unwrap();
    assert!(plain["capabilities"].get("supports_sampling_params").is_none());
    assert!(plain["capabilities"].get("supports_thinking").is_none());
}
