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
use sqlx::PgPool;

async fn open_pool(server: &crate::common::TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test DB")
}

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

#[tokio::test]
async fn test_migration_drops_legacy_summarizer_columns_from_memory_admin_settings() {
    // Migration 91 explicitly DROPs the 4 summarizer columns from
    // `memory_admin_settings`. A SELECT for any of them must fail with
    // `42703 undefined_column`. Drift here would silently regress
    // documentation contracts in the migration script itself.
    let server = crate::common::TestServer::start().await;
    let pool = open_pool(&server).await;

    for col in [
        "summarize_after_tokens",
        "summarizer_keep_recent_tokens",
        "full_summary_prompt",
        "incremental_summary_prompt",
    ] {
        let q = format!("SELECT {col} FROM memory_admin_settings LIMIT 1");
        let result = sqlx::query(&q).execute(&pool).await;
        let err = result
            .expect_err(&format!("memory_admin_settings.{col} should be dropped"));
        let s = err.to_string();
        assert!(
            s.contains("does not exist") || s.contains("42703"),
            "expected undefined_column on dropped {col}, got: {s}"
        );
    }
}

#[tokio::test]
async fn test_keep_lt_trigger_check_constraint_fires() {
    // Direct SQL UPDATE that sets `summarizer_keep_recent_tokens >=
    // summarize_after_tokens` must fail the `summarizer_keep_lt_trigger`
    // CHECK — the handler validates this too (test_keep_recent_must_be_below_trigger
    // in admin_settings_test.rs), but the DB-level CHECK is the backstop
    // for direct-SQL writers and the constraint name is part of the
    // migration contract.
    let server = crate::common::TestServer::start().await;
    let pool = open_pool(&server).await;

    let result = sqlx::query(
        "UPDATE summarization_admin_settings
            SET summarize_after_tokens = 5000,
                summarizer_keep_recent_tokens = 5000
            WHERE id = 1",
    )
    .execute(&pool)
    .await;

    let err = result.expect_err("keep == trigger must violate the CHECK");
    let s = err.to_string();
    assert!(
        s.contains("summarizer_keep_lt_trigger") || s.contains("check"),
        "expected the summarizer_keep_lt_trigger CHECK to fire, got: {s}"
    );
}
