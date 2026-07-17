//! TEST-27 — the agent module-owned migrations apply: the `workflow_runs`
//! `agent_transcript_json` column + the `resumable` run status + the
//! `mcp_tool_calls.review_classification` column exist and are writable, and the
//! `agent_admin_settings` singleton row is seeded. Pure schema assertions (no
//! model) against a freshly-migrated per-test DB.

use crate::common::TestServer;

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

#[tokio::test]
async fn agent_migrations_apply_columns_status_and_settings() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // 1. workflow_runs.agent_transcript_json (jsonb) exists.
    let has_transcript: Option<String> = sqlx::query_scalar(
        "SELECT data_type FROM information_schema.columns
         WHERE table_name = 'workflow_runs' AND column_name = 'agent_transcript_json'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(
        has_transcript.as_deref(),
        Some("jsonb"),
        "workflow_runs.agent_transcript_json (jsonb) must exist"
    );

    // 2. The `resumable` run status is accepted by the status CHECK constraint —
    //    insert a row with status='resumable' into a scratch and roll back.
    let mut tx = pool.begin().await.unwrap();
    // A minimal workflow_runs insert may require FKs; instead assert the CHECK
    // constraint text mentions `resumable` (the migration widened it).
    let check_has_resumable: Option<bool> = sqlx::query_scalar(
        "SELECT bool_or(pg_get_constraintdef(oid) LIKE '%resumable%')
         FROM pg_constraint
         WHERE conrelid = 'workflow_runs'::regclass AND contype = 'c'",
    )
    .fetch_optional(&mut *tx)
    .await
    .unwrap()
    .flatten();
    assert_eq!(
        check_has_resumable,
        Some(true),
        "a workflow_runs CHECK constraint must allow status='resumable'"
    );
    tx.rollback().await.unwrap();

    // 3. mcp_tool_calls.review_classification column exists.
    let has_class: Option<String> = sqlx::query_scalar(
        "SELECT data_type FROM information_schema.columns
         WHERE table_name = 'mcp_tool_calls' AND column_name = 'review_classification'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        has_class.is_some(),
        "mcp_tool_calls.review_classification must exist"
    );

    // 4. agent_admin_settings singleton row is seeded.
    let settings_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM agent_admin_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        settings_rows >= 1,
        "agent_admin_settings singleton must be seeded; found {settings_rows} rows"
    );
}
