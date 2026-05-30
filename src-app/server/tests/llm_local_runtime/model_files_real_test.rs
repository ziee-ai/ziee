//! Tier 2 — REAL model file lifecycle (network; needs HUGGINGFACE_API_KEY).
//!
//! Per the agreed test plan, real HuggingFace download runs in the
//! default suite. This exercises the real download pipeline end-to-end
//! (initiate → poll → commit → model row + files).
//!
//! Real multipart UPLOAD is a 1500-LoC multi-step session flow; it is
//! covered end-to-end by the Playwright E2E spec
//! `12-local-runtime/09-model-upload.spec.ts` (the real production UI
//! path) rather than re-implementing the multipart contract here.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use uuid::Uuid;

/// Download `hf-internal-testing/tiny-random-gpt2` (a few MB) into a
/// local provider and assert the model row materializes.
#[tokio::test]
async fn real_hf_download_creates_model() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let provider = lrt::create_test_provider(&server, &admin.token, "real-dl-provider").await;
    let provider_id = Uuid::parse_str(provider["id"].as_str().unwrap()).unwrap();

    // Reuses the proven download helper: initiates the HF download and
    // polls to completion (panics on timeout/failure).
    let (model, _path) = lrt::download_test_model(&server, &admin.token, provider_id).await;

    assert!(model["id"].as_str().is_some(), "downloaded model should have an id");
    assert_eq!(model["provider_id"].as_str(), Some(provider_id.to_string().as_str()));

    // Capabilities JSONB exists on the model (auto-detection may populate
    // it from the repo's config.json; we assert it's at least present).
    let model_id = model["id"].as_str().unwrap();
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/{model_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("capabilities").is_some(),
        "model row should carry a capabilities field: {body}"
    );
}
