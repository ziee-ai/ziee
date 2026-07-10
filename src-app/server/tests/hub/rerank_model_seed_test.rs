//! TEST-9 (ITEM-10,2): the vendored hub seed carries the BGE reranker model with
//! `capabilities.rerank=true`, so the seed→hub→llm_model capability path exposes
//! it. The seed source lives in the TRACKED `resources/hub-seed` (copied into the
//! `include_dir!`-baked `binaries/hub-seed` at build), so this is not release-gated.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

#[tokio::test]
async fn seed_catalog_exposes_the_reranker_model_with_rerank_capability() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "hub_rr", &["hub::models::read"]).await;

    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/hub/models?lang=en"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap()
        .json().await.unwrap();
    let models = body.as_array().expect("models array");

    let reranker = models.iter().find(|m| {
        m["name"].as_str().map(|n| n.contains("bge-reranker")).unwrap_or(false)
    });
    let reranker = reranker.unwrap_or_else(|| panic!(
        "the seed catalog must contain the bge-reranker model; names: {:?}",
        models.iter().filter_map(|m| m["name"].as_str()).collect::<Vec<_>>()
    ));
    assert_eq!(
        reranker["capabilities"]["rerank"], serde_json::json!(true),
        "the reranker model exposes capabilities.rerank=true: {reranker}"
    );
}
