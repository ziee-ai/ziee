// ============================================================================
// `GET /api/llm-models?capability=<name>` filter tests.
//
// Plan §8: "GET /api/llm-models with capability=text_embedding filter
// — small SQL filter on capabilities->>'text_embedding' = 'true'".
// ============================================================================

use serde_json::Value;

#[tokio::test]
async fn test_capability_filter_rejects_unknown_value() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "cap_unknown",
        &["llm_models::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/llm-models?capability=NOT_A_REAL_CAPABILITY"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "unknown capability must be rejected");
}

#[tokio::test]
async fn test_capability_filter_accepts_text_embedding() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "cap_te",
        &["llm_models::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/llm-models?capability=text_embedding&page=1&per_page=10"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body.get("models").is_some(), "response must have models array");
    // All returned models (if any) must actually carry text_embedding=true.
    let models = body["models"].as_array().expect("models array");
    for m in models {
        assert_eq!(
            m["capabilities"]["text_embedding"].as_bool().unwrap_or(false),
            true,
            "every filtered model must have text_embedding=true: {}",
            m
        );
    }
}
