//! `POST /workflow-runs/{run_id}/elicit/{elicitation_id}` validation
//! paths. Drives a REAL pending elicitation (so the schema-valid case
//! reaches the in-process runner waiter → 200/delivered):
//!
//!   - the workflow is a single `kind: elicit` step (no upstream llm
//!     step to mock — elicit blocks immediately on run start);
//!   - we `POST /run`, then poll `GET /workflow-runs/{id}` until
//!     `pending_elicitation_json` is set by the ElicitDispatcher;
//!   - then exercise: (a) wrong user → 403, (b) stale elicitation_id →
//!     410, (c) schema-invalid response → 422, (d) schema-valid → 200.
//!
//! Both the runner task and the elicit endpoint live in the SAME server
//! process, so the in-process registry waiter the runner registers is
//! reachable by the endpoint — that's what lets (d) deliver and resume
//! the run to `completed`.
//!
//! NOTE (uncertain — not runnable here): the exact timing of when
//! `pending_elicitation_json` becomes visible vs. when the run is
//! cleaned up is best-effort; `poll_pending_elicitation` retries with a
//! short deadline. If this proves flaky under real execution, the
//! fallback documented in the plan §7 is to hand-insert a workflow_runs
//! row with a crafted pending_elicitation_json via a direct PgPool
//! (TestServer exposes `database_url`) and test the 403/410/422 paths
//! only (the 200 path needs a live runner and cannot be hand-faked).

use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// A single-step elicit workflow. `message` is the prompt (shared
/// `StepDef.message` field — NOT nested under the elicit config). The
/// schema requires a boolean `proceed`.
const ELICIT_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: confirm
    kind: elicit
    message: "Proceed with {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        proceed:
          type: boolean
          title: "Proceed?"
      required: [proceed]
    timeout_ms: 300000
outputs:
  - name: decision
    from: "{{ confirm.output }}"
"#;

/// Poll `GET /workflow-runs/{id}` until `pending_elicitation_json` is
/// non-null, returning the `elicitation_id`. Panics on timeout.
async fn poll_pending_elicitation(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
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
        if let Some(pending) = run["pending_elicitation_json"].as_object() {
            if let Some(id) = pending.get("elicitation_id").and_then(|v| v.as_str()) {
                return Uuid::parse_str(id).expect("elicitation_id uuid");
            }
        }
        // If the run already terminated without ever pausing, that's a
        // setup failure — surface it.
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "failed" | "cancelled" | "completed") {
            panic!("run {run_id} reached terminal '{status}' before pausing on elicit: {run}");
        }
        if Instant::now() >= deadline {
            panic!("run {run_id} never set pending_elicitation_json: {run}");
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// Helper: import + run a single-step elicit workflow → returns
/// (workflow_user_token, run_id, elicitation_id).
async fn start_paused_elicit_run(server: &TestServer, slug: &str) -> (String, Uuid, Uuid) {
    let user = workflow_user(server, &format!("elicit_{slug}")).await;
    let wf = import_dev_workflow(server, &user.token, slug, ELICIT_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let (_stub, conv_id) = stub_conversation(server, &user.user_id, &user.token).await;
    // NOTE: _stub is dropped here — fine, because the elicit step never
    // dispatches to a provider; the model snapshot is taken at run start
    // (spawn_run) before the stub guard drops.

    let run = run_workflow(
        server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "shipping the feature" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let elicitation_id = poll_pending_elicitation(server, &user.token, run_id).await;
    (user.token, run_id, elicitation_id)
}

#[tokio::test]
async fn elicit_wrong_user_is_forbidden() {
    let server = plain_server().await;
    let (_owner_token, run_id, elicitation_id) = start_paused_elicit_run(&server, "owner-403").await;

    // A different user with workflow perms tries to answer → 403.
    let other = create_user_with_permissions(
        &server,
        "elicit_intruder",
        &["workflows::read", "workflows::execute"],
    )
    .await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "response": { "proceed": true } }))
        .send()
        .await
        .expect("elicit wrong user");
    assert_eq!(
        resp.status(),
        403,
        "another user must not answer this run's elicitation: {}",
        resp.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn set_run_timeout_updates_for_owner_and_forbids_other_user() {
    // A paused-on-elicit run is in-flight (its registry handle is live), so the
    // live-timeout endpoint (PUT /workflow-runs/{id}/timeout) can adjust it.
    let server = plain_server().await;
    let (owner_token, run_id, _elicitation_id) =
        start_paused_elicit_run(&server, "timeout-live").await;
    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/workflow-runs/{run_id}/timeout"));

    // Owner lifts the wall-clock cap (secs: 0 = unbounded) → 200 { status: "updated" }.
    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {owner_token}"))
        .json(&json!({ "timeout_secs": 0 }))
        .send()
        .await
        .expect("owner set timeout");
    assert_eq!(resp.status(), 200, "owner may set their own run's timeout");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "updated", "in-flight run timeout updated: {body}");
    assert_eq!(body["run_id"], run_id.to_string());

    // An over-ceiling value is accepted (clamped server-side) → still "updated".
    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {owner_token}"))
        .json(&json!({ "timeout_secs": 99_999_999_999u64 }))
        .send()
        .await
        .expect("owner clamp timeout");
    assert_eq!(resp.status(), 200, "an over-ceiling timeout is clamped, not rejected");

    // A different user must NOT adjust this run's timeout → 403.
    let other = create_user_with_permissions(
        &server,
        "timeout_intruder",
        &["workflows::read", "workflows::execute"],
    )
    .await;
    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "timeout_secs": 600 }))
        .send()
        .await
        .expect("other user set timeout");
    assert_eq!(
        resp.status(),
        403,
        "another user cannot change this run's timeout: {}",
        resp.text().await.unwrap_or_default()
    );

    // Drive the run to a terminal state, then PUT /timeout → 200 {already_terminal}:
    // the registry handle is gone, so there's nothing live to adjust.
    let cancel = client
        .post(server.api_url(&format!("/workflow-runs/{run_id}/cancel")))
        .header("Authorization", format!("Bearer {owner_token}"))
        .send()
        .await
        .expect("cancel run");
    assert_eq!(cancel.status(), 200, "owner cancels their own run");
    let final_run = poll_run(&server, &owner_token, run_id).await;
    assert_eq!(final_run["status"], "cancelled", "run reached terminal: {final_run}");

    // The registry handle is unregistered by the runner task AFTER the DB flips
    // to cancelled, so there's a brief window where set_timeout still finds a
    // live handle ("updated"). Poll until the handle is gone → "already_terminal".
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let resp = client
            .put(&url)
            .header("Authorization", format!("Bearer {owner_token}"))
            .json(&json!({ "timeout_secs": 0 }))
            .send()
            .await
            .expect("set timeout on terminal run");
        assert_eq!(resp.status(), 200, "PUT /timeout on a finished run still 200s");
        let body: serde_json::Value = resp.json().await.unwrap();
        let status = body["status"].as_str().unwrap_or("");
        if status == "already_terminal" {
            break;
        }
        assert_eq!(status, "updated", "only 'updated'/'already_terminal' expected: {body}");
        assert!(
            Instant::now() < deadline,
            "handle was never reaped after the run went terminal"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn elicit_stale_id_is_gone() {
    let server = plain_server().await;
    let (owner_token, run_id, _elicitation_id) = start_paused_elicit_run(&server, "stale-410").await;

    // A random (non-matching) elicitation_id → 410 Gone.
    let stale = Uuid::new_v4();
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{stale}")))
        .header("Authorization", format!("Bearer {owner_token}"))
        .json(&json!({ "response": { "proceed": true } }))
        .send()
        .await
        .expect("elicit stale");
    assert_eq!(
        resp.status(),
        410,
        "a stale elicitation_id must 410 Gone: {}",
        resp.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn elicit_schema_invalid_response_is_unprocessable() {
    let server = plain_server().await;
    let (owner_token, run_id, elicitation_id) = start_paused_elicit_run(&server, "schema-422").await;

    // Response violates the schema: `proceed` must be a boolean.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {owner_token}"))
        .json(&json!({ "response": { "proceed": "not-a-boolean" } }))
        .send()
        .await
        .expect("elicit schema-invalid");
    assert_eq!(
        resp.status(),
        422,
        "a schema-mismatched response must 422: {}",
        resp.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn elicit_schema_valid_response_delivers_and_resumes() {
    let server = plain_server().await;
    let (owner_token, run_id, elicitation_id) = start_paused_elicit_run(&server, "valid-200").await;

    // A schema-valid response → 200, delivered to the in-process runner.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {owner_token}"))
        .json(&json!({ "response": { "proceed": true } }))
        .send()
        .await
        .expect("elicit valid");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse ack");
    assert_eq!(status, 200, "schema-valid elicit should 200: {body}");
    assert_eq!(body["status"], "delivered", "ack reports delivered: {body}");
    assert_eq!(
        body["elicitation_id"],
        elicitation_id.to_string(),
        "ack echoes the elicitation_id: {body}"
    );

    // The run resumes + completes (single-step elicit workflow).
    let final_run = poll_run(&server, &owner_token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "run resumes + completes after the elicit reply: {final_run}"
    );
}

/// A single-step elicit workflow with a SHORT, BOUNDED wall-clock
/// (`timeout_ms: 2500`). Used to exercise the bounded-gate EXPIRY path
/// in `dispatch.rs` (the `tokio::time::sleep_until(deadline)` arm that
/// fails the step with "elicit timed out after {ms}ms") — distinct from
/// the durable `timeout_ms: 0` gate (resume.rs) and the long-lived
/// `timeout_ms: 300000` gate that is always answered/cancelled before it
/// can fire (the other tests in this file).
const SHORT_TIMEOUT_ELICIT_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: confirm
    kind: elicit
    message: "Proceed with {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        proceed:
          type: boolean
          title: "Proceed?"
      required: [proceed]
    timeout_ms: 2500
outputs:
  - name: decision
    from: "{{ confirm.output }}"
"#;

/// A BOUNDED elicit gate (`timeout_ms > 0`) that is NEVER answered must
/// fire its wall-clock and FAIL the run — the bounded-gate expiry arm in
/// `dispatch.rs` (`error: "elicit timed out after {timeout_ms}ms"` →
/// RunFailed). The existing coverage stops short of this: resume.rs
/// drives the `timeout_ms: 0` DURABLE gate (which never wall-clocks), and
/// the rest of this file uses `timeout_ms: 300000` and always answers or
/// cancels before the timeout could fire. This asserts the timeout
/// actually firing.
#[tokio::test]
async fn bounded_elicit_gate_times_out_and_fails_run() {
    let server = plain_server().await;
    let user = workflow_user(&server, "elicit_timeout_fires").await;
    let wf = import_dev_workflow(&server, &user.token, "elicit-timeout", SHORT_TIMEOUT_ELICIT_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "shipping the feature" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // Prove it genuinely PARKS on the elicit first (registers a pending
    // elicitation) — so the subsequent failure is the wall-clock firing,
    // not a setup error. The 2.5s gate is comfortably longer than the
    // poll cadence, so the pending state is observable before expiry.
    let _elicitation_id = poll_pending_elicitation(&server, &user.token, run_id).await;

    // Do NOT answer. The bounded wall-clock fires (~2.5s) → the step
    // errors and the run transitions to `failed`.
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "failed",
        "an unanswered bounded elicit gate must fail the run when its timeout fires: {final_run}"
    );
    let err = final_run["error_message"].as_str().unwrap_or("");
    assert!(
        err.contains("timed out") || err.contains("timeout"),
        "the failure must reference the elicit timeout (dispatch.rs bounded-gate expiry): {final_run}"
    );
}
