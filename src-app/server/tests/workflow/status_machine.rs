//! Audit gap-closure D1–D5 — the workflow run status machine + startup sweep.
//!
//! The audit flagged that several `workflow::repository` / `startup_sweep`
//! functions are only exercised indirectly (via the HTTP lifecycle) or
//! re-implemented as inline SQL inside a single durability test
//! (`access_and_durability::cancelled_run_survives_late_completion`), and
//! never asserted against the REAL function. This file closes those gaps by
//! calling the production functions directly:
//!
//! - D1 `fail_orphaned_runs`  — orphan-run crash recovery + the M-3 cutoff
//!                            race-guard (a run started inside the boot window
//!                            is NOT swept).
//! - D2 `mark_status` CAS    — terminal-write guard (a late terminal write
//!                            against a terminal row is a no-op).
//! - D3 `mark_running` /     — the per-transition status guards.
//!     `cancel_cas` / `heartbeat`
//! - D4 `/cancel` endpoint    — the cancel HTTP path against a `running` run.
//! - D5 `persist_step_meta`   — the `jsonb_set` MERGE (not overwrite) across
//!                            two step ids + the saturating u64→i64 token cast.
//!
//! The real fns are reached via the `ziee::workflow` re-export added to
//! `src/lib.rs` (mirrors the `ziee::workflow_mcp` / `ziee::code_sandbox`
//! re-export blocks the sibling repository tests already rely on). The free
//! `fail_orphaned_runs` takes a `time::OffsetDateTime`; the integration-test
//! crate has no `time` crate in scope, so D1 calls the chrono-free
//! `fail_orphaned_runs_before_unix` wrapper (it converts a unix-epoch `i64`
//! cutoff to `time::OffsetDateTime` inside the server crate and forwards to the
//! real fn). Because `mark_status`/`mark_running`/`heartbeat` return
//! `Result<()>` (not `rows_affected`), the CAS no-op cases are asserted by the
//! resulting row STATUS (the behavior the audit cares about), not a row count.

use serde_json::{Value as Json, json};
use uuid::Uuid;

use ziee::workflow::{
    cancel_cas, fail_orphaned_runs_before_unix, heartbeat, mark_running, mark_status,
    persist_step_meta, workflow_workspace_root, WorkflowRunStatus,
};

use super::{
    SIMPLE_OK_YAML, db_pool, import_dev_workflow, plain_server, poll_run, run_workflow,
    stub_model_for, workflow_user,
};

/// Insert a run row in an explicit `status` (default `created_at = NOW()`),
/// returning its id. Mirrors `run_history_and_delete.rs`'s direct-SQL insert.
async fn insert_run_with_status(
    pool: &sqlx::PgPool,
    workflow_id: Uuid,
    user_id: Uuid,
    status: &str,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(workflow_id)
    .bind(user_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("insert {status} run: {e}"))
}

/// Read the current `status` of a run row.
async fn run_status(pool: &sqlx::PgPool, run_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT status FROM workflow_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("read run status")
}

/// Read `updated_at` as a unix-epoch microsecond `i64` (the test crate has no
/// `time` feature on sqlx; epoch-micros via SQL avoids binding a timestamp
/// type while still proving monotonic advance / no-touch).
async fn updated_at_micros(pool: &sqlx::PgPool, run_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT (EXTRACT(EPOCH FROM updated_at) * 1000000)::bigint \
         FROM workflow_runs WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("read updated_at micros")
}

// ── D1: startup_sweep crash recovery + M-3 cutoff race-guard ──────────────────

#[tokio::test]
async fn startup_sweep_fails_orphans_older_than_cutoff_but_spares_boot_window() {
    // `startup_sweep::sweep_at_boot` calls `repository::fail_orphaned_runs`,
    // whose SQL flips every pending/running run with `created_at < cutoff` to
    // `failed`. The `created_at < cutoff` bound (M-3) spares a run that was
    // legitimately started in the boot window AFTER the cutoff was captured.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d1_sweep").await;
    let wf = import_dev_workflow(&server, &user.token, "d1-sweep", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    // Orphan: a `running` run created well in the past (5 minutes ago) — older
    // than any plausible cutoff, so the sweep must reclaim it.
    let orphan_running: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status, created_at) \
         VALUES ($1, $2, 'running', NOW() - INTERVAL '5 minutes') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await
    .expect("insert old running run");

    // Orphan: an old `pending` run — the sweep covers `('pending','running')`.
    let orphan_pending: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status, created_at) \
         VALUES ($1, $2, 'pending', NOW() - INTERVAL '5 minutes') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await
    .expect("insert old pending run");

    // Boot-window run: a freshly-created `running` run (created_at = NOW()),
    // INSIDE the race-guard window — must NOT be flipped (M-3).
    let fresh_running = insert_run_with_status(&db, wf_id, user_uuid, "running").await;

    // Control: an already-terminal `completed` run must be untouched.
    let completed = insert_run_with_status(&db, wf_id, user_uuid, "completed").await;

    // Capture the cutoff between the old orphans and the fresh run: 1 minute in
    // the past is older than NOW() (so `fresh_running` survives) but newer than
    // the 5-minutes-ago orphans (so they are swept). This is exactly the
    // boot-time `now - boot_grace` cutoff `sweep_at_boot` is fed.
    let cutoff_unix: i64 =
        sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM (NOW() - INTERVAL '1 minute'))::bigint")
            .fetch_one(&db)
            .await
            .expect("compute cutoff");

    // Drive the REAL orphan flip (via the chrono-free test wrapper).
    let affected = fail_orphaned_runs_before_unix(&db, cutoff_unix)
        .await
        .expect("fail_orphaned_runs");

    assert_eq!(
        affected, 2,
        "exactly the two pre-cutoff orphans (running + pending) must be swept; got {affected}"
    );
    assert_eq!(
        run_status(&db, orphan_running).await,
        "failed",
        "an old orphaned `running` run must be flipped to `failed` (D1 crash recovery)"
    );
    assert_eq!(
        run_status(&db, orphan_pending).await,
        "failed",
        "an old orphaned `pending` run must be flipped to `failed` (D1 crash recovery)"
    );
    assert_eq!(
        run_status(&db, fresh_running).await,
        "running",
        "a run created INSIDE the boot window (created_at >= cutoff) must NOT be swept (M-3)"
    );
    assert_eq!(
        run_status(&db, completed).await,
        "completed",
        "an already-terminal run must be left untouched by the sweep"
    );

    // The error_message stamped by the sweep is the operator-facing reason.
    let msg: Option<String> =
        sqlx::query_scalar("SELECT error_message FROM workflow_runs WHERE id = $1")
            .bind(orphan_running)
            .fetch_one(&db)
            .await
            .expect("read swept error_message");
    assert_eq!(
        msg.as_deref(),
        Some("server restart during execution"),
        "the sweep must stamp the restart reason on swept orphans"
    );

    // NOTE: `sweep_at_boot`'s SECOND phase (walk `<workspace>/*/workflow/*/`
    // and `remove_dir_all` staged dirs whose run_id is no longer non-terminal,
    // skipping non-UUID dir names) is exercised only indirectly here: it
    // operates on the live server's workspace root, which a test process must
    // not race against. The DB orphan-flip above (the part the dir-reclamation
    // re-checks per run) is the testable precondition; the `fs::remove_dir_all`
    // walk itself is left to the boot path.

    db.close().await;
}

// ── D2: mark_status terminal-write CAS (the REAL fn) ───────────────────────────

#[tokio::test]
async fn mark_status_cas_first_terminal_writer_wins() {
    // `repository::mark_status` guards terminal writes:
    //   WHERE status NOT IN ('cancelled','completed','failed')
    //         OR ($allow_cancelled_self AND status='cancelled')
    // so the first terminal writer wins and a later writer is a no-op. The fn
    // returns `Result<()>` (not a row count), so we assert the resulting STATUS.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d2_cas").await;
    let wf = import_dev_workflow(&server, &user.token, "d2-cas", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    // (a) running → cancelled: the guard allows it (status is non-terminal).
    let run = insert_run_with_status(&db, wf_id, user_uuid, "running").await;
    mark_status(&db, run, WorkflowRunStatus::Cancelled, Some("cancelled by user"))
        .await
        .expect("mark_status running→cancelled");
    assert_eq!(
        run_status(&db, run).await,
        "cancelled",
        "running → cancelled must persist `cancelled` (D2)"
    );

    // (b) cancelled → completed: a late `Completed` write must be a no-op and
    //     leave the status `cancelled` — the audit's core case.
    mark_status(&db, run, WorkflowRunStatus::Completed, None)
        .await
        .expect("mark_status (no-op) returns Ok");
    assert_eq!(
        run_status(&db, run).await,
        "cancelled",
        "an already-cancelled run must stay `cancelled` after a late completion (D2)"
    );

    // (c) cancelled → cancelled: the `allow_cancelled_self` branch lets the
    //     runner's idempotent Cancelled re-assert match the row, still leaving
    //     it `cancelled` (never resurrects a completed/failed run).
    mark_status(&db, run, WorkflowRunStatus::Cancelled, Some("cancelled by user"))
        .await
        .expect("mark_status re-assert cancelled");
    assert_eq!(run_status(&db, run).await, "cancelled", "still cancelled (D2)");

    // (d) completed → cancelled: the special-case must NOT resurrect an
    //     already-`completed` run to `cancelled` (self-cancel only applies when
    //     the row is ALREADY cancelled).
    let done = insert_run_with_status(&db, wf_id, user_uuid, "completed").await;
    mark_status(&db, done, WorkflowRunStatus::Cancelled, Some("cancelled by user"))
        .await
        .expect("mark_status (no-op) returns Ok");
    assert_eq!(
        run_status(&db, done).await,
        "completed",
        "a completed run must NOT be cancellable via mark_status (no resurrection) (D2)"
    );

    db.close().await;
}

// ── D3: mark_running / cancel_cas / heartbeat per-transition guards ────────────

#[tokio::test]
async fn mark_running_only_promotes_pending() {
    // `repository::mark_running`: UPDATE … SET status='running'
    //   WHERE id=$1 AND status='pending'. Returns Ok regardless of match.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d3_running").await;
    let wf = import_dev_workflow(&server, &user.token, "d3-running", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    // pending → running: allowed.
    let pending = insert_run_with_status(&db, wf_id, user_uuid, "pending").await;
    mark_running(&db, pending).await.expect("mark_running pending");
    assert_eq!(run_status(&db, pending).await, "running", "pending → running (D3)");

    // cancelled → running: blocked — a fast cancel that beat the runner to
    // `mark_running` must NOT be resurrected to `running`.
    let cancelled = insert_run_with_status(&db, wf_id, user_uuid, "cancelled").await;
    mark_running(&db, cancelled)
        .await
        .expect("mark_running (no-op) returns Ok");
    assert_eq!(
        run_status(&db, cancelled).await,
        "cancelled",
        "a cancelled run must NOT be promoted to running past a late mark_running (D3)"
    );

    db.close().await;
}

#[tokio::test]
async fn cancel_cas_only_cancels_cancelable_states() {
    // `repository::cancel_cas`: UPDATE … SET status='cancelled'
    //   WHERE id=$1 AND status IN ('pending','running') RETURNING status.
    // `RETURNING` is evaluated AFTER `SET`, so on a successful flip it yields
    // the new value (`cancelled`); on a non-cancelable row it yields None.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d3_cancel").await;
    let wf = import_dev_workflow(&server, &user.token, "d3-cancel", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    // running → cancelled: returns the NEW status row (`cancelled`); flip ok.
    let running = insert_run_with_status(&db, wf_id, user_uuid, "running").await;
    let returned = cancel_cas(&db, running).await.expect("cancel_cas running");
    assert_eq!(
        returned.as_deref(),
        Some("cancelled"),
        "cancel_cas on a running run must return a row (the cancel succeeded) (D3)"
    );
    assert_eq!(run_status(&db, running).await, "cancelled", "now cancelled (D3)");

    // pending → cancelled: also cancelable.
    let pending = insert_run_with_status(&db, wf_id, user_uuid, "pending").await;
    assert!(
        cancel_cas(&db, pending).await.expect("cancel_cas pending").is_some(),
        "cancel_cas on a pending run must succeed (D3)"
    );
    assert_eq!(run_status(&db, pending).await, "cancelled", "now cancelled (D3)");

    // completed → (no-op): RETURNING yields None — nothing to cancel.
    let completed = insert_run_with_status(&db, wf_id, user_uuid, "completed").await;
    assert!(
        cancel_cas(&db, completed).await.expect("cancel_cas completed").is_none(),
        "cancel_cas on a completed run must be a no-op (None) (D3)"
    );
    assert_eq!(
        run_status(&db, completed).await,
        "completed",
        "a completed run is untouched by cancel_cas (D3)"
    );

    db.close().await;
}

#[tokio::test]
async fn heartbeat_only_bumps_non_terminal_runs() {
    // `repository::heartbeat`: UPDATE … SET updated_at=NOW()
    //   WHERE id=$1 AND status IN ('pending','running'). Bumps `updated_at`
    //   (liveness) WITHOUT changing status, and only while non-terminal.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d3_heartbeat").await;
    let wf = import_dev_workflow(&server, &user.token, "d3-heartbeat", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    // Insert a running run with a deliberately stale updated_at, then beat it.
    let running: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status, updated_at) \
         VALUES ($1, $2, 'running', NOW() - INTERVAL '10 minutes') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await
    .expect("insert stale running run");

    let before = updated_at_micros(&db, running).await;
    heartbeat(&db, running).await.expect("heartbeat running");
    let after = updated_at_micros(&db, running).await;
    assert!(
        after > before,
        "heartbeat must advance updated_at (before={before}, after={after}) (D3)"
    );
    assert_eq!(
        run_status(&db, running).await,
        "running",
        "heartbeat must NOT change status (D3)"
    );

    // A terminal (cancelled) run is never touched by the heartbeat.
    let cancelled = insert_run_with_status(&db, wf_id, user_uuid, "cancelled").await;
    let c_before = updated_at_micros(&db, cancelled).await;
    heartbeat(&db, cancelled)
        .await
        .expect("heartbeat (no-op) returns Ok");
    let c_after = updated_at_micros(&db, cancelled).await;
    assert_eq!(
        c_before, c_after,
        "heartbeat must NOT bump a terminal run's updated_at (D3)"
    );

    db.close().await;
}

// ── D4: /workflow-runs/{id}/cancel endpoint e2e (the REAL handler) ─────────────

#[tokio::test]
async fn cancel_endpoint_cancels_a_running_run() {
    // The full HTTP path: POST /workflow-runs/{id}/cancel → cancel_run handler
    // → repository::cancel_cas → GET shows `cancelled`. We insert a `running`
    // row directly (mirroring `delete_non_terminal_run_is_rejected`) so the
    // cancel acts on a deterministically non-terminal run rather than racing a
    // fast-completing mock run.
    //
    // Ack semantics: `cancel_cas`'s `RETURNING status` is AFTER the `SET`, so
    // it returns the NEW value (`cancelled`) on success, None on a no-op; the
    // handler maps that to `CancelAckResponse.status` (`already_terminal` when
    // None). So a first cancel of a running run acks `cancelled`; a re-cancel
    // acks `already_terminal`.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d4_cancel").await;
    let wf = import_dev_workflow(&server, &user.token, "d4-cancel", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let pool = db_pool(&server).await;
    let run_id = insert_run_with_status(&pool, wf_id, user_uuid, "running").await;
    pool.close().await;

    // POST /cancel → 200 OK; ack echoes the post-cancel status.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/cancel")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("cancel run");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse cancel ack");
    assert!(status.is_success(), "cancel must 2xx; got {status}: {body}");
    assert_eq!(
        body["status"], "cancelled",
        "cancel ack must echo the post-cancel status (cancel_cas RETURNs the new value): {body}"
    );

    // GET /workflow-runs/{id} → status now `cancelled`.
    let run: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get run after cancel")
        .json()
        .await
        .expect("parse run after cancel");
    assert_eq!(
        run["status"], "cancelled",
        "the run must read `cancelled` after the cancel endpoint (D4): {run}"
    );

    // Idempotent re-cancel: a second POST is a no-op (cancel_cas returns None,
    // the handler reports `already_terminal`) and the row stays cancelled.
    let resp2 = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/cancel")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("re-cancel run");
    let status2 = resp2.status();
    let body2: Json = resp2.json().await.expect("parse re-cancel ack");
    assert!(status2.is_success(), "re-cancel must 2xx; got {status2}: {body2}");
    assert_eq!(
        body2["status"], "already_terminal",
        "a re-cancel of a terminal run reports already_terminal (D4): {body2}"
    );
}

// ── D5: persist_step_meta jsonb_set MERGE + saturating u64→i64 token cast ──────

#[tokio::test]
async fn persist_step_meta_merges_steps_and_accumulates_tokens() {
    // `repository::persist_step_meta` writes step metadata into
    // `step_outputs_json[step_id]` via
    // `jsonb_set(coalesce(...,'{}'), [step], $3, true)` and adds
    // `total_tokens + i64::try_from(delta).unwrap_or(i64::MAX)`. Two calls for
    // DIFFERENT step ids must MERGE (both keys present), not overwrite; and an
    // absurd u64 token delta must saturate to i64::MAX.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_d5_meta").await;
    let wf = import_dev_workflow(&server, &user.token, "d5-meta", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;
    let run_id = insert_run_with_status(&db, wf_id, user_uuid, "running").await;

    // First step.
    persist_step_meta(
        &db,
        run_id,
        "research",
        &json!({ "kind": "json", "size_bytes": 100, "preview": "first" }),
        42,
        Some("research"),
    )
    .await
    .expect("persist_step_meta research");

    // Second step (different id) — must MERGE, not overwrite the first.
    persist_step_meta(
        &db,
        run_id,
        "summarize",
        &json!({ "kind": "json", "size_bytes": 200, "preview": "second" }),
        58,
        Some("summarize"),
    )
    .await
    .expect("persist_step_meta summarize");

    // Both step keys present (the jsonb_set merge accumulated them).
    let outputs: Json =
        sqlx::query_scalar("SELECT step_outputs_json FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("read step_outputs_json");
    assert!(
        outputs.get("research").is_some(),
        "step_outputs_json must retain the FIRST step after a second write (jsonb merge): {outputs}"
    );
    assert!(
        outputs.get("summarize").is_some(),
        "step_outputs_json must include the SECOND step: {outputs}"
    );
    assert_eq!(
        outputs["research"]["preview"], "first",
        "the first step's metadata must survive the merge intact (D5): {outputs}"
    );
    assert_eq!(
        outputs["summarize"]["preview"], "second",
        "the second step's metadata must be present (D5): {outputs}"
    );

    // total_tokens accumulated across both calls (42 + 58 = 100), and
    // current_step advanced to the latest write.
    let total: i64 = sqlx::query_scalar("SELECT total_tokens FROM workflow_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&db)
        .await
        .expect("read total_tokens");
    assert_eq!(
        total, 100,
        "total_tokens must accumulate the two deltas (42 + 58), not overwrite (D5)"
    );
    let current: Option<String> =
        sqlx::query_scalar("SELECT current_step FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("read current_step");
    assert_eq!(
        current.as_deref(),
        Some("summarize"),
        "current_step must advance to the latest persisted step (D5)"
    );

    // Saturating cast: a u64 delta larger than i64::MAX must clamp to i64::MAX,
    // not wrap negative. On a fresh run (total starts at 0) total_tokens must
    // become exactly i64::MAX.
    let saturate_run = insert_run_with_status(&db, wf_id, user_uuid, "running").await;
    let absurd_delta: u64 = (i64::MAX as u64) + 1;
    persist_step_meta(
        &db,
        saturate_run,
        "huge",
        &json!({ "kind": "text" }),
        absurd_delta,
        None,
    )
    .await
    .expect("persist_step_meta saturating");
    let saturated: i64 =
        sqlx::query_scalar("SELECT total_tokens FROM workflow_runs WHERE id = $1")
            .bind(saturate_run)
            .fetch_one(&db)
            .await
            .expect("read saturated total_tokens");
    assert_eq!(
        saturated,
        i64::MAX,
        "a u64 token delta exceeding i64::MAX must saturate to i64::MAX, not wrap (D5)"
    );

    db.close().await;
}

// ── A7 durability: captured logs are persisted into step_logs_json ─────────────

#[tokio::test]
async fn captured_step_logs_persist_to_step_logs_json() {
    // Audit fix: `repository::persist_step_logs` previously had NO caller, so
    // `step_logs_json` stayed empty and `read_log`'s durable DB fallback (used
    // after the staging dir is GC'd on restart / by the 30-day reaper) was
    // dead. The runner now snapshots a step's on-disk logs into
    // `step_logs_json` on completion. A completed step always writes a `trace`,
    // so assert that trace body landed in the column.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_durable_logs").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;
    let wf = import_dev_workflow(&server, &user.token, "durable-logs", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "x" },
            "model_id": model_id.to_string(),
            "mocks": { "gen": "hi" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completes: {final_run}");

    let db = db_pool(&server).await;
    let logs: Json = sqlx::query_scalar("SELECT step_logs_json FROM workflow_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&db)
        .await
        .expect("read step_logs_json");
    db.close().await;

    // The `gen` step's trace must be persisted WITH a non-empty body — that's
    // exactly what read_log serves from the DB once the on-disk file is gone.
    let body = logs
        .get("gen")
        .and_then(|s| s.get("trace"))
        .and_then(|t| t.get("body"))
        .and_then(|b| b.as_str());
    assert!(
        body.is_some_and(|b| !b.is_empty()),
        "step_logs_json must persist the completed step's trace body (A7 durable fallback): {logs}"
    );
}

#[tokio::test]
async fn durable_logs_served_from_db_after_staging_dir_removed() {
    // The end-to-end point of the persist_step_logs fix: once the on-disk
    // staging dir is reclaimed (server restart / 30-day reaper), read_log falls
    // back to the durable step_logs_json body. Run a step, DELETE its on-disk
    // logs, then read_log must STILL serve the trace.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_log_fallback").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;
    let wf = import_dev_workflow(&server, &user.token, "log-fallback", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "x" },
            "model_id": model_id.to_string(),
            "mocks": { "gen": "hi" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completes: {final_run}");

    // Simulate staging-dir GC: a standalone run keys its workspace by run_id, so
    // logs live at <root>/<run>/workflow/<run>/logs/.
    let logs_dir = workflow_workspace_root()
        .join(run_id.to_string())
        .join("workflow")
        .join(run_id.to_string())
        .join("logs");
    assert!(
        logs_dir.exists(),
        "on-disk logs must exist before removal: {}",
        logs_dir.display()
    );
    std::fs::remove_dir_all(&logs_dir).expect("remove staged logs (simulated GC)");

    // read_log now MUST serve the trace from the durable step_logs_json body.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/logs/gen/trace")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read log after GC");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 200,
        "trace must be served from the durable DB fallback after staging-dir GC: {body}"
    );
    assert!(
        !body.trim().is_empty(),
        "the durable fallback body must be non-empty"
    );
}
