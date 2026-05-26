// ============================================================================
// Summarizer scaffolding (plan §9 Phase 6).
//
// The full summarization roundtrip requires:
//   1. A long branch (50+ messages) — needs the chat module's full
//      message-create + branching primitives + a real LLM.
//   2. An extraction model configured in memory_admin_settings.
//
// The full E2E lives in a Tier-5 manual exercise. This unit-style
// scaffold drives just the public surface: the `apply_summary_to_history`
// helper is exposed via the memory chat extension; we assert the
// /api/memory/admin-settings.default_extraction_model_id round-trips
// (used by the summarizer's auto-refresh path).
// ============================================================================

use serde_json::{Value, json};

#[tokio::test]
async fn test_default_extraction_model_round_trip() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_admin",
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;

    // Set a fake (not really existing) model id via the PATCH endpoint
    // — the schema allows NULL, so we test the round-trip with NULL
    // (no real models seeded in this lightweight test).
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_extraction_model_id": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["default_extraction_model_id"].is_null());
}
