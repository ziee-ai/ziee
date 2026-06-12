// ============================================================================
// Migration 91 seed contract.
//
// Documents the on-by-default + compiled-default-thresholds story so a
// future migration that changes either value forces a deliberate
// test update (drift = audible). Per the locked design decision, the
// migration does NOT copy values from `memory_admin_settings` —
// every deployment lands at the compiled defaults regardless of
// prior memory configuration.
// ============================================================================

use serde_json::Value;

#[tokio::test]
async fn test_migration_seeds_enabled_true_and_compiled_defaults() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_seed",
        &["summarization::settings::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();

    // On by default — zero-config compaction benefits every deployment.
    assert_eq!(row["enabled"], true, "migration 91 seeds enabled=TRUE");

    // No summarizer model picked — the chat extension falls back to the
    // conversation's own model id when this is NULL.
    assert!(
        row["default_summarization_model_id"].is_null(),
        "seed leaves default_summarization_model_id NULL for zero-config fallback"
    );

    // Compiled-in token thresholds — match the migration's DEFAULTs.
    assert_eq!(row["summarize_after_tokens"], 12000);
    assert_eq!(row["summarizer_keep_recent_tokens"], 3000);

    // NULL prompt overrides — the engine uses its compiled-in default
    // prompts unless an admin sets an override.
    assert!(row["full_summary_prompt"].is_null());
    assert!(row["incremental_summary_prompt"].is_null());
}
