//! Durable workflow resume (Change B) + unbounded elicit timeout (Change A).
//!
//! A `timeout_ms: 0` elicit gate is a DURABLE checkpoint: the runner persists
//! the pending record, flips the run to `waiting`, and SUSPENDS (the task
//! exits — no resident block, no wall-clock ticking, survives a restart). When
//! the user submits, `submit_elicit` finds no resident handle, persists the
//! response, and spawns `runner::resume_run`, which rehydrates completed-step
//! outputs, SKIPS them, consumes the response at the gate, and continues.
//!
//! The headline test proves the whole chain in one run, including a definitive
//! "completed step is NOT re-run" check: after the run parks, we overwrite the
//! pre-gate step's persisted output file with a SENTINEL. If resume skips +
//! rehydrates, the final output reflects the sentinel; a re-run would instead
//! reflect the original mock. So `result_a == SENTINEL` proves skip+rehydrate.

use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use uuid::Uuid;

use ziee::workflow::fail_orphaned_runs_before_unix;

use super::{
    db_pool, import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation,
    stub_model_for, workflow_user,
};
use crate::common::TestServer;

/// 3-step workflow: a (llm, mocked) → gate (elicit, `timeout_ms: 0`, durable) →
/// b (llm, mocked). Outputs reference a (skip proof), b (resume-continues
/// proof), and the gate response (consume proof).
const RESUME_WORKFLOW_YAML: &str = r#"inputs:
  - name: topic
    required: true
steps:
  - id: a
    kind: llm
    output_format: json
    prompt: "describe {{ inputs.topic }}"
  - id: gate
    kind: elicit
    depends_on: [a]
    message: "approve {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        approved:
          type: boolean
      required: [approved]
    timeout_ms: 0
  - id: b
    kind: llm
    depends_on: [gate]
    prompt: "finalize {{ inputs.topic }}"
outputs:
  - name: result_a
    from: "{{ a.output.marker }}"
  - name: result_b
    from: "{{ b.output }}"
  - name: gate_resp
    from: "{{ gate.output.approved }}"
"#;

/// Poll `GET /workflow-runs/{id}` until `status == "waiting"` (parked on the
/// durable gate), returning the pending `elicitation_id`. Panics on timeout or
/// an early terminal status.
async fn poll_until_waiting(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let run: Json = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("get run")
            .json()
            .await
            .expect("parse run");
        let status = run["status"].as_str().unwrap_or("");
        if status == "waiting" {
            let eid = run["pending_elicitation_json"]["elicitation_id"]
                .as_str()
                .expect("waiting run must carry a pending elicitation_id");
            return Uuid::parse_str(eid).expect("elicitation_id uuid");
        }
        if matches!(status, "failed" | "cancelled" | "completed") {
            panic!("run {run_id} reached terminal '{status}' before parking: {run}");
        }
        if Instant::now() >= deadline {
            panic!("run {run_id} never reached `waiting`: {run}");
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

async fn submit_elicit(
    server: &TestServer,
    token: &str,
    run_id: Uuid,
    elicitation_id: Uuid,
    response: Json,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "response": response }))
        .send()
        .await
        .expect("submit elicit")
}

#[tokio::test]
async fn durable_gate_suspends_spared_by_sweep_and_resumes_skipping_completed() {
    let server = plain_server().await;
    let user = workflow_user(&server, "resume_headline").await;
    // Standalone run (explicit model_id, no conversation) so the workspace is
    // keyed by run_id — and so resume can re-resolve the model from the row.
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;
    let wf = import_dev_workflow(&server, &user.token, "durable-resume", RESUME_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id");

    // Run with mocks for the two llm steps; the gate is NOT mocked so it really
    // dispatches and suspends.
    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "durable execution" },
            "model_id": model_id.to_string(),
            "mocks": { "a": { "marker": "ORIGINAL" }, "b": "B_DONE" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // ── Park: the gate suspends the run to `waiting`. ──
    let elicitation_id = poll_until_waiting(&server, &user.token, run_id).await;

    let db = db_pool(&server).await;
    let parked: Json = sqlx::query_scalar::<_, Json>(
        "SELECT to_jsonb(r) FROM workflow_runs r WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&db)
    .await
    .expect("read parked run");
    assert_eq!(parked["status"], "waiting", "durable gate parks the run: {parked}");
    assert!(
        parked["step_outputs_json"].get("a").is_some(),
        "the pre-gate step `a` ran before the suspend: {parked}"
    );
    assert!(
        parked["step_outputs_json"].get("gate").is_none()
            && parked["step_outputs_json"].get("b").is_none(),
        "the gate and post-gate step have no output yet: {parked}"
    );
    // a's persisted output host path — used for the sentinel overwrite below.
    let a_path = parked["step_outputs_json"]["a"]["path"]
        .as_str()
        .expect("a output path")
        .to_string();

    // ── Restart sparing: the boot sweep must NOT fail a `waiting` run. Use a
    //    far-future cutoff so it WOULD reclaim any pending/running orphan. ──
    let future_cutoff: i64 =
        sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM (NOW() + INTERVAL '1 hour'))::bigint")
            .fetch_one(&db)
            .await
            .expect("compute future cutoff");
    fail_orphaned_runs_before_unix(&db, future_cutoff)
        .await
        .expect("sweep");
    let after_sweep: String =
        sqlx::query_scalar("SELECT status FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("status after sweep");
    assert_eq!(
        after_sweep, "waiting",
        "the boot sweep must SPARE a durable `waiting` run (resumable, not failed)"
    );

    // ── Skip proof: overwrite a's persisted output with a sentinel. If resume
    //    SKIPS a and rehydrates its meta, the final output reflects the
    //    sentinel; a re-run would overwrite the file with the mock again. ──
    std::fs::write(&a_path, br#"{"marker":"SENTINEL_NOT_RERUN"}"#)
        .expect("overwrite a output with sentinel");

    // ── Resume: submit the durable gate. No resident runner exists (the run
    //    suspended), so this drives the cold path → resume_run. ──
    let resp = submit_elicit(&server, &user.token, run_id, elicitation_id, json!({ "approved": true })).await;
    let status = resp.status();
    let ack: Json = resp.json().await.expect("parse ack");
    assert_eq!(status, 200, "cold-resume submit must 200: {ack}");
    assert_eq!(ack["status"], "delivered", "ack reports delivered: {ack}");

    // ── Completes after resume. ──
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the run resumes from the gate and completes: {final_run}"
    );

    // result_a == sentinel → `a` was SKIPPED + rehydrated (NOT re-run).
    assert_eq!(
        final_run["final_output_json"]["result_a"]["value_preview"], "SENTINEL_NOT_RERUN",
        "completed step `a` must be skipped + rehydrated on resume, not re-run: {final_run}"
    );
    // result_b == mock → the post-gate step ran (resume continued the DAG).
    assert_eq!(
        final_run["final_output_json"]["result_b"]["value_preview"], "B_DONE",
        "the post-gate step `b` must run after resume: {final_run}"
    );
    // gate_resp == submitted value → the gate consumed the durable response.
    assert_eq!(
        final_run["final_output_json"]["gate_resp"]["value_preview"], "true",
        "the gate must resolve with the submitted response on resume: {final_run}"
    );
    // All three steps now have persisted outputs.
    for step in ["a", "gate", "b"] {
        assert!(
            final_run["step_outputs_json"].get(step).is_some(),
            "step `{step}` must have a persisted output after completion: {final_run}"
        );
    }

    // ── Double-submit after resolution → 410 GONE (pending was cleared). ──
    let resp2 = submit_elicit(&server, &user.token, run_id, elicitation_id, json!({ "approved": true })).await;
    assert_eq!(
        resp2.status(),
        410,
        "a second submit after the gate resolved must be GONE: {}",
        resp2.text().await.unwrap_or_default()
    );

    db.close().await;
}

#[tokio::test]
async fn migration_check_accepts_waiting_status() {
    // Migration 110 widened the workflow_runs.status CHECK to include
    // 'waiting'. An insert with status='waiting' must succeed; an unknown
    // status must still be rejected by the constraint.
    let server = plain_server().await;
    let user = workflow_user(&server, "resume_migration").await;
    let wf = import_dev_workflow(&server, &user.token, "mig-check", super::SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = db_pool(&server).await;

    let ok: Result<Uuid, _> = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, 'waiting') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await;
    assert!(ok.is_ok(), "status='waiting' must satisfy the CHECK constraint: {ok:?}");

    let bad: Result<Uuid, _> = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, 'bogus') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await;
    assert!(bad.is_err(), "an unknown status must still be rejected by the CHECK");

    db.close().await;
}

#[tokio::test]
async fn cold_waiting_run_can_be_cancelled() {
    // A durable gate parks the run `waiting` with NO resident runner. The
    // cancel endpoint (cancel_cas, widened to include 'waiting') must still
    // cancel it.
    let server = plain_server().await;
    let user = workflow_user(&server, "resume_cancel").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;
    let wf = import_dev_workflow(&server, &user.token, "cold-cancel", RESUME_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "cancel me" },
            "model_id": model_id.to_string(),
            "mocks": { "a": { "marker": "ORIGINAL" }, "b": "B_DONE" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let _ = poll_until_waiting(&server, &user.token, run_id).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/cancel")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("cancel cold waiting run");
    let status = resp.status();
    let ack: Json = resp.json().await.expect("parse cancel ack");
    assert!(status.is_success(), "cancel of a cold waiting run must 2xx; got {status}: {ack}");
    assert_eq!(
        ack["status"], "cancelled",
        "cancel_cas must flip a `waiting` run to cancelled: {ack}"
    );

    let db = db_pool(&server).await;
    let final_status: String =
        sqlx::query_scalar("SELECT status FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("status after cancel");
    db.close().await;
    assert_eq!(final_status, "cancelled", "the waiting run must read cancelled after /cancel");
}

/// Multi-step CONVERSATION-BOUND run that pauses on an elicit gate MID-RUN,
/// then resumes to completion once answered. The existing resume coverage uses
/// a STANDALONE run (explicit model_id); conversation-bound elicit was only
/// exercised on the happy path in real_stack (real LLM). Here the model is
/// derived from the conversation, the two llm steps are mocked (deterministic,
/// no API key), and only the gate really suspends — proving the a → gate(park)
/// → answer → b → complete sequence works when the run is bound to a chat.
#[tokio::test]
async fn conversation_bound_multistep_elicit_parks_and_resumes() {
    let server = plain_server().await;
    let user = workflow_user(&server, "resume_conv").await;
    // Stub model + a conversation bound to it (no API key needed).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let wf = import_dev_workflow(&server, &user.token, "durable-resume-conv", RESUME_WORKFLOW_YAML)
        .await;
    let wf_id = wf["id"].as_str().expect("workflow id");

    // Conversation-bound run: the runner snapshots the model from the
    // conversation (NOT an explicit model_id). The gate is not mocked.
    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "conversation-bound resume" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "a": { "marker": "ORIGINAL" }, "b": "B_DONE" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // ── Park mid-run on the elicit gate. ──
    let elicitation_id = poll_until_waiting(&server, &user.token, run_id).await;

    // Pre-gate step `a` ran; the gate + post-gate step `b` have not yet.
    let db = db_pool(&server).await;
    let parked: Json = sqlx::query_scalar::<_, Json>(
        "SELECT to_jsonb(r) FROM workflow_runs r WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&db)
    .await
    .expect("read parked run");
    assert_eq!(parked["status"], "waiting", "gate parks the conversation-bound run: {parked}");
    assert!(parked["step_outputs_json"].get("a").is_some(), "pre-gate step a ran: {parked}");
    assert!(parked["step_outputs_json"].get("b").is_none(), "post-gate step b not yet: {parked}");

    // ── Answer the elicit → run resumes and finishes step `b`. ──
    let resp = submit_elicit(
        &server,
        &user.token,
        run_id,
        elicitation_id,
        json!({ "approved": true }),
    )
    .await;
    assert!(resp.status().is_success(), "submit elicit should succeed: {}", resp.status());

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "resumed run must complete: {final_run}");
    // All three steps have outputs now (a pre-gate, gate consumed, b post-gate).
    let outs = &final_run["step_outputs_json"];
    assert!(outs.get("a").is_some(), "a present after resume: {final_run}");
    assert!(outs.get("gate").is_some(), "gate response recorded: {final_run}");
    assert!(outs.get("b").is_some(), "post-gate step b ran on resume: {final_run}");
    db.close().await;
}
