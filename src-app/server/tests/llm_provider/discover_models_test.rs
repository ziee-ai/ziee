//! TEST-7 (ITEM-7, ITEM-5): `GET /llm-providers/{id}/discover-models` enriches
//! live models with context / vision / tools parsed from a rich (OpenRouter-shaped)
//! `/models` response, drops pricing, and is permission-gated.
//!
//! The live fetch normally uses `PUBLIC_HTTP_OR_HTTPS` (blocks loopback); the
//! debug-only `LLM_DISCOVER_ALLOW_LOOPBACK=1` seam relaxes it to `DEV_LOCAL` so a
//! 127.0.0.1 wiremock stands in for the upstream.

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::test_helpers::{
    create_user_with_only_permissions, create_user_with_permissions,
};
use crate::common::{TestServer, TestServerOptions};

async fn create_provider(
    server: &TestServer,
    token: &str,
    provider_type: &str,
    base_url: &str,
) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": format!("{provider_type}-{}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": provider_type,
            "base_url": base_url,
            "api_key": "test-key",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "provider create should 201");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn discover_enriches_openrouter_models_and_gates_permission() {
    // OpenRouter-shaped /models with a vision + tool-capable model.
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{
                "id": "vendor/vision-tool-model",
                "name": "Vendor Vision",
                "context_length": 200000,
                "architecture": { "input_modalities": ["text", "image"] },
                "supported_parameters": ["tools", "temperature", "max_tokens"],
                "top_provider": { "max_completion_tokens": 64000 },
                "pricing": { "prompt": "0.001", "completion": "0.002" }
            }]
        })))
        .mount(&upstream)
        .await;

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LLM_DISCOVER_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;

    let admin = create_user_with_permissions(
        &server,
        "discover_admin",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // openrouter has NO curated-catalog entries, so the discover response is
    // purely the enriched live models — a clean assertion surface.
    let provider_id = create_provider(&server, &admin.token, "openrouter", &upstream.uri()).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{provider_id}/discover-models")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();

    let models = body["models"].as_array().expect("models array");
    let m = models
        .iter()
        .find(|m| m["id"] == "vendor/vision-tool-model")
        .expect("live model present in discovery");

    assert_eq!(m["context_length"].as_u64(), Some(200000), "context parsed");
    assert_eq!(m["max_output_tokens"].as_u64(), Some(64000), "max output parsed");
    assert_eq!(m["supports_vision"].as_bool(), Some(true), "image modality → vision");
    assert_eq!(m["supports_tool_use"].as_bool(), Some(true), "tools param → tool_use");
    assert_eq!(m["display_name"].as_str(), Some("Vendor Vision"));
    assert_eq!(m["source"].as_str(), Some("discovery"));
    // pricing must NEVER surface in a discovered model (DEC-4).
    assert!(m.get("pricing").is_none(), "pricing must be dropped");

    // Permission gate: a user without llm_providers::read is refused. Use
    // `only_permissions` so no default-group read perm leaks in.
    let no_read =
        create_user_with_only_permissions(&server, "discover_noread", &["profile::read"]).await;
    let forbidden = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{provider_id}/discover-models")))
        .header("Authorization", format!("Bearer {}", no_read.token))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}
