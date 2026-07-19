//! TEST-95/97 — the durable `agent_task_list` table behind the agent-core
//! `TaskListStore` port (Group G / DEC-49/50) persists + reads back per-run task
//! items, keyed purely by `run_id` (structural cross-run isolation), and enforces
//! its status CHECK vocabulary.
//!
//! The port impl (`PgTaskListStore`) lives behind the server's private `modules`
//! tree, so this exercises the exact SQL shape the store issues (the `position`
//! append subquery, the status vocabulary, the deps jsonb, the run-scoped
//! read-back + patch) against a freshly-migrated per-test DB — proving the
//! migration + the schema the store depends on.

use uuid::Uuid;

use crate::common::TestServer;

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Insert one item the way `PgTaskListStore::create` does (append-at-end
/// `position`, status vocabulary, deps jsonb), returning its id.
async fn insert_item(
    pool: &sqlx::PgPool,
    run_id: Uuid,
    content: &str,
    active_form: &str,
    status: &str,
    deps: serde_json::Value,
) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO agent_task_list
            (run_id, content, active_form, status, owner, deps, position)
        VALUES (
            $1, $2, $3, $4, NULL, $5,
            COALESCE((SELECT MAX(position) + 1 FROM agent_task_list WHERE run_id = $1), 0)
        )
        RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(content)
    .bind(active_form)
    .bind(status)
    .bind(deps)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn agent_task_list_persists_reads_back_and_isolates_by_run() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    let run_a = Uuid::new_v4();
    let run_b = Uuid::new_v4();

    // Two items for run A (creation order preserved by `position`), one for run B.
    let dep = Uuid::new_v4();
    let a0 = insert_item(&pool, run_a, "Run tests", "Running tests", "in_progress", serde_json::json!([])).await;
    let _a1 = insert_item(
        &pool,
        run_a,
        "Write report",
        "Writing report",
        "pending",
        serde_json::json!([dep.to_string()]),
    )
    .await;
    let _b0 = insert_item(&pool, run_b, "Other run", "Other running", "pending", serde_json::json!([])).await;

    // Read-back for run A: exactly its two items, in creation order.
    let rows: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT content, status, deps FROM agent_task_list WHERE run_id = $1 ORDER BY position ASC, created_at ASC",
    )
    .bind(run_a)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "run A must see ONLY its own two items (run isolation)");
    assert_eq!(rows[0].0, "Run tests");
    assert_eq!(rows[0].1, "in_progress");
    assert_eq!(rows[1].0, "Write report");
    // deps jsonb round-trips as a string array carrying the dep uuid.
    assert_eq!(
        rows[1].2,
        serde_json::json!([dep.to_string()]),
        "deps jsonb must persist the dependency uuid"
    );

    // Patch item a0 → completed (the CC "mark complete immediately" transition).
    sqlx::query("UPDATE agent_task_list SET status = 'completed', updated_at = NOW() WHERE run_id = $1 AND id = $2")
        .bind(run_a)
        .bind(a0)
        .execute(&pool)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM agent_task_list WHERE id = $1")
        .bind(a0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed", "the status patch must persist");

    // The status CHECK rejects an out-of-vocabulary value (fail-closed schema).
    let bad = sqlx::query(
        "INSERT INTO agent_task_list (run_id, content, active_form, status) VALUES ($1, 'x', 'x', 'bogus')",
    )
    .bind(run_a)
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "an unknown status must be rejected by the CHECK constraint");
}
