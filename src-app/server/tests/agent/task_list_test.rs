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
//!
//! Plus the run-terminal reconciliation suite (fix/agent-tasklist-reconciliation):
//! it drives the REAL terminal writers (`workflow_mark_status` / `workflow_cancel_cas`)
//! and reconcile fns (`reconcile_run_terminal` / `workflow_reconcile_orphaned_task_lists`)
//! exposed via `ziee::test_internals`, and asserts open task rows are driven to
//! `abandoned` at every terminal path while completed rows are left untouched.

use uuid::Uuid;

use crate::common::TestServer;
use ziee::test_internals::{
    WorkflowRunStatus, reconcile_run_terminal, workflow_cancel_cas, workflow_fail_orphaned_runs,
    workflow_mark_status, workflow_reconcile_orphaned_task_lists, workflow_sweep_at_boot,
};

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Insert one item the way `PgTaskListStore::create` does — append-at-end
/// `position`, status vocabulary, deps jsonb, AND the existence-guarded
/// `workflow_run_id` subquery (NULL unless run_id is a real workflow_runs row).
/// Returns its id.
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
            (run_id, content, active_form, status, owner, deps, position, workflow_run_id)
        VALUES (
            $1, $2, $3, $4, NULL, $5,
            COALESCE((SELECT MAX(position) + 1 FROM agent_task_list WHERE run_id = $1), 0),
            (SELECT id FROM workflow_runs WHERE id = $1)
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

/// Create a real `workflow_runs` row (satisfying its NOT NULL FKs to `workflows`
/// + `users`) in the given status, so the terminal writers actually flip a row.
/// Returns the run id (= the task rows' `run_id` key for workflow/background).
async fn make_workflow_run(pool: &sqlx::PgPool, status: &str) -> Uuid {
    // Own the user (the per-test DB is migrated but not seeded with one) — only
    // username + email are NOT NULL on `users`.
    let uniq = &Uuid::new_v4().to_string()[..8];
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("recon_{uniq}"))
            .bind(format!("recon_{uniq}@example.com"))
            .fetch_one(pool)
            .await
            .expect("create test user");
    let workflow_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO workflows
            (name, extracted_path, bundle_sha256, bundle_size_bytes, file_count,
             entry_point, scope, owner_user_id)
        VALUES ('recon-test', '/tmp/recon', 'sha', 0, 0, 'main.yaml', 'user', $1)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(workflow_id)
    .bind(user_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn status_of(pool: &sqlx::PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM agent_task_list WHERE id = $1")
        .bind(id)
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
    let a0 = insert_item(
        &pool,
        run_a,
        "Run tests",
        "Running tests",
        "in_progress",
        serde_json::json!([]),
    )
    .await;
    let _a1 = insert_item(
        &pool,
        run_a,
        "Write report",
        "Writing report",
        "pending",
        serde_json::json!([dep.to_string()]),
    )
    .await;
    let _b0 = insert_item(
        &pool,
        run_b,
        "Other run",
        "Other running",
        "pending",
        serde_json::json!([]),
    )
    .await;

    // Read-back for run A: exactly its two items, in creation order.
    let rows: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT content, status, deps FROM agent_task_list WHERE run_id = $1 ORDER BY position ASC, created_at ASC",
    )
    .bind(run_a)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "run A must see ONLY its own two items (run isolation)"
    );
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
    assert_eq!(
        status_of(&pool, a0).await,
        "completed",
        "the status patch must persist"
    );

    // The status CHECK rejects an out-of-vocabulary value (fail-closed schema).
    let bad = sqlx::query(
        "INSERT INTO agent_task_list (run_id, content, active_form, status) VALUES ($1, 'x', 'x', 'bogus')",
    )
    .bind(run_a)
    .execute(&pool)
    .await;
    assert!(
        bad.is_err(),
        "an unknown status must be rejected by the CHECK constraint"
    );
}

/// TEST-1 [acceptance INV-1] — the literal rig reproduction: a run driven terminal
/// via the REAL `mark_status(Completed)` drives its open task rows to `abandoned`
/// while its completed row is preserved. RED before the reconcile hook existed.
#[tokio::test]
async fn reconcile_marks_open_rows_abandoned_on_completion() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let run = make_workflow_run(&pool, "running").await;

    let pending = insert_item(
        &pool,
        run,
        "Write report",
        "Writing report",
        "pending",
        serde_json::json!([]),
    )
    .await;
    let in_prog = insert_item(
        &pool,
        run,
        "Run tests",
        "Running tests",
        "in_progress",
        serde_json::json!([]),
    )
    .await;
    let done = insert_item(
        &pool,
        run,
        "Set up repo",
        "Setting up repo",
        "completed",
        serde_json::json!([]),
    )
    .await;

    workflow_mark_status(&pool, run, WorkflowRunStatus::Completed, None)
        .await
        .unwrap();

    assert_eq!(
        status_of(&pool, pending).await,
        "abandoned",
        "pending → abandoned at run end"
    );
    assert_eq!(
        status_of(&pool, in_prog).await,
        "abandoned",
        "in_progress → abandoned at run end"
    );
    assert_eq!(
        status_of(&pool, done).await,
        "completed",
        "completed rows are preserved (never re-labelled)"
    );
}

/// TEST-2 [acceptance INV-2] — the counterpart: a run terminating with ALL rows
/// already completed is left byte-for-byte untouched (no completed row is ever
/// rewritten to abandoned; reconciliation never claims unfinished work was done).
#[tokio::test]
async fn reconcile_leaves_all_completed_run_untouched() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let run = make_workflow_run(&pool, "running").await;

    // Two completed rows AND one open row, so the hook MUST fire (discriminating:
    // if the hook were deleted, `open` would stay in_progress; if reconcile wrongly
    // included 'completed' in its WHERE, d0/d1 would flip — either mutation → RED).
    let d0 = insert_item(&pool, run, "A", "a", "completed", serde_json::json!([])).await;
    let d1 = insert_item(&pool, run, "B", "b", "completed", serde_json::json!([])).await;
    let open = insert_item(&pool, run, "C", "c", "in_progress", serde_json::json!([])).await;

    workflow_mark_status(&pool, run, WorkflowRunStatus::Completed, None)
        .await
        .unwrap();

    // INV-2: completed rows are NEVER rewritten, even when the hook fires.
    assert_eq!(
        status_of(&pool, d0).await,
        "completed",
        "completed row preserved"
    );
    assert_eq!(
        status_of(&pool, d1).await,
        "completed",
        "completed row preserved"
    );
    assert_eq!(
        status_of(&pool, open).await,
        "abandoned",
        "the open row is the only one reconciled"
    );
    let abandoned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_task_list WHERE run_id = $1 AND status = 'abandoned'",
    )
    .bind(run)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        abandoned, 1,
        "exactly the one open row abandons; completed rows must NOT be re-labelled"
    );
}

/// TEST-6 — the fail terminal arm: `mark_status(Failed)` also reconciles (proves
/// the hook fires for EVERY terminal status, not just Completed).
#[tokio::test]
async fn reconcile_marks_open_rows_abandoned_on_failure() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let run = make_workflow_run(&pool, "running").await;
    let open = insert_item(
        &pool,
        run,
        "Half-done",
        "Doing half",
        "in_progress",
        serde_json::json!([]),
    )
    .await;

    workflow_mark_status(&pool, run, WorkflowRunStatus::Failed, Some("boom"))
        .await
        .unwrap();

    assert_eq!(
        status_of(&pool, open).await,
        "abandoned",
        "in_progress → abandoned on a FAILED run"
    );
}

/// TEST-3 [acceptance INV-3] — the NON-happy terminal paths (where the leak came
/// from): (a) the user-cancel path `cancel_cas`; (b) the crash/restart-recovery
/// path (`fail_orphaned_runs` + the boot bulk sweep); and (c) retroactive
/// remediation of a pre-existing leak under an already-`completed` run.
#[tokio::test]
async fn reconcile_covers_cancel_crash_and_retroactive_paths() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // (a) user cancel — cancel_cas bypasses mark_status but must still reconcile.
    let cancelled = make_workflow_run(&pool, "running").await;
    let c_open = insert_item(
        &pool,
        cancelled,
        "Cancel me",
        "Cancelling",
        "in_progress",
        serde_json::json!([]),
    )
    .await;
    let prior = workflow_cancel_cas(&pool, cancelled).await.unwrap();
    assert!(prior.is_some(), "cancel must land on a running run");
    assert_eq!(
        status_of(&pool, c_open).await,
        "abandoned",
        "cancel → open rows abandoned"
    );

    // (b) crash/restart recovery: an orphaned 'running' run swept to 'failed',
    // then the boot bulk sweep abandons its open task rows.
    let orphan = make_workflow_run(&pool, "running").await;
    let o_open = insert_item(
        &pool,
        orphan,
        "Orphaned",
        "Orphaning",
        "pending",
        serde_json::json!([]),
    )
    .await;
    // cutoff in the future so the just-created run (created_at = now) is swept.
    let cutoff = time::OffsetDateTime::now_utc() + time::Duration::minutes(1);
    workflow_fail_orphaned_runs(&pool, cutoff).await.unwrap();
    let orphan_status: String =
        sqlx::query_scalar("SELECT status FROM workflow_runs WHERE id = $1")
            .bind(orphan)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        orphan_status, "failed",
        "startup sweep fails the orphaned run"
    );
    let swept = workflow_reconcile_orphaned_task_lists(&pool).await.unwrap();
    assert!(
        swept >= 1,
        "boot bulk sweep reconciles the failed run's open rows"
    );
    assert_eq!(status_of(&pool, o_open).await, "abandoned");

    // (c) retroactive: a run ALREADY terminal (completed) with a leaked open row
    // (the rig's 35 completed-run cases) is remediated by the same bulk sweep.
    let leaked = make_workflow_run(&pool, "completed").await;
    let l_open = insert_item(
        &pool,
        leaked,
        "Stuck forever",
        "Stuck",
        "in_progress",
        serde_json::json!([]),
    )
    .await;
    workflow_reconcile_orphaned_task_lists(&pool).await.unwrap();
    assert_eq!(
        status_of(&pool, l_open).await,
        "abandoned",
        "retroactive: stuck row under a completed run is healed"
    );
}

/// TEST-4 [acceptance INV-4] — the FK: `workflow_run_id` is populated only for a
/// real workflow run (NULL for a chat-shaped message-id run_id, no FK violation),
/// and deleting the `workflow_runs` row CASCADE-removes its task rows (no orphan).
#[tokio::test]
async fn workflow_run_id_fk_populates_and_cascades() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // Chat-shaped run_id (not a workflow_runs row): insert must succeed, FK NULL.
    let chat_run = Uuid::new_v4();
    let chat_item = insert_item(
        &pool,
        chat_run,
        "chat task",
        "chatting",
        "pending",
        serde_json::json!([]),
    )
    .await;
    let chat_fk: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM agent_task_list WHERE id = $1")
            .bind(chat_item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        chat_fk.is_none(),
        "a non-workflow run_id leaves workflow_run_id NULL (no FK violation)"
    );

    // Workflow run_id: workflow_run_id is set = run_id.
    let run = make_workflow_run(&pool, "running").await;
    let wf_item = insert_item(
        &pool,
        run,
        "wf task",
        "working",
        "pending",
        serde_json::json!([]),
    )
    .await;
    let wf_fk: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM agent_task_list WHERE id = $1")
            .bind(wf_item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        wf_fk,
        Some(run),
        "a real workflow run_id populates workflow_run_id via the guarded subquery"
    );

    // Deleting the run CASCADE-removes its task rows (the orphan the table deferred).
    sqlx::query("DELETE FROM workflow_runs WHERE id = $1")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_task_list WHERE run_id = $1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining, 0,
        "ON DELETE CASCADE removes the run's task rows"
    );
}

/// TEST-5 (unit-shaped, integration-hosted) — the reconcile primitive is keyed by
/// the UNIVERSAL run_id, so it also abandons rows for a non-workflow (chat-shaped)
/// run_id, and leaves a completed row untouched. Proves the primitive independent
/// of the workflow terminal writers.
#[tokio::test]
async fn reconcile_run_terminal_primitive_is_run_id_keyed() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let run = Uuid::new_v4(); // chat-shaped key (no workflow_runs row)

    let open = insert_item(
        &pool,
        run,
        "open",
        "opening",
        "pending",
        serde_json::json!([]),
    )
    .await;
    let done = insert_item(
        &pool,
        run,
        "done",
        "doing",
        "completed",
        serde_json::json!([]),
    )
    .await;

    let n = reconcile_run_terminal(&pool, run).await.unwrap();
    assert_eq!(n, 1, "exactly the one open row is reconciled");
    assert_eq!(status_of(&pool, open).await, "abandoned");
    assert_eq!(status_of(&pool, done).await, "completed");
    // Idempotent: a second call flips nothing.
    assert_eq!(
        reconcile_run_terminal(&pool, run).await.unwrap(),
        0,
        "reconcile is idempotent"
    );
}

/// TEST-3 (wiring) — proves the PRODUCTION boot wiring, not just the repo fn:
/// `sweep_at_boot` itself must reconcile open task rows under terminal runs.
/// Deleting the `reconcile_orphaned_task_lists` call inside `sweep_at_boot` turns
/// this RED (the direct-fn assertion in TEST-3 would still pass).
#[tokio::test]
async fn sweep_at_boot_reconciles_orphaned_task_rows() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    // A run already terminal (completed) with a leaked open row.
    let leaked = make_workflow_run(&pool, "completed").await;
    let open = insert_item(
        &pool,
        leaked,
        "Stuck",
        "Stuck",
        "in_progress",
        serde_json::json!([]),
    )
    .await;

    // Drive the REAL boot entry point (its fs cleanup is a no-op when the
    // workspace root is absent). cutoff in the past → no live run is swept.
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::minutes(1);
    workflow_sweep_at_boot(&pool, cutoff).await.unwrap();

    assert_eq!(
        status_of(&pool, open).await,
        "abandoned",
        "sweep_at_boot must reconcile the leaked row (production wiring)"
    );
}
