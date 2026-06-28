//! Conversation-bound, multi-step workflow run with an elicit MID-RUN that
//! resumes to completion — the DETERMINISTIC counterpart to the two existing
//! elicit tests (audit `all-650153fb52ff`).
//!
//! Coverage gap this closes:
//!   - `resume.rs::durable_gate_suspends_..._resumes_skipping_completed` proves
//!     multi-step + mid-run elicit + completion, but for a STANDALONE run
//!     (explicit `model_id`, `conversation_id = NULL`).
//!   - `real_stack.rs::real_stack_combined_all_kinds_completes...` proves a
//!     CONVERSATION-BOUND mid-run elicit, but only on the REAL-LLM + sandbox
//!     path (hard-skips without a provider key / rootfs) and only the single
//!     happy answer.
//!
//! Neither gives a key-free, sandbox-free test that a CONVERSATION-BOUND run
//! with steps BEFORE and AFTER an elicit pauses, accepts a mid-run answer, and
//! continues the DAG to completion — re-resolving its model from the bound
//! conversation on resume. This does, with the two `llm` steps mocked (so it
//! runs in CI with no API key and no sandbox), keeping the elicit dispatch +
//! pause/resume entirely real.

use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    db_pool, import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation,
    workflow_user,
};
use crate::common::TestServer;

/// 3-step workflow: `a` (llm, mocked) → `gate` (elicit, resident timeout) →
/// `b` (llm, mocked). The gate sits in the MIDDLE — `a` runs before it, `b`
/// after — so completing the run proves the elicit did not truncate the DAG.
const CONV_ELICIT_WORKFLOW_YAML: &str = r#"inputs:
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
    timeout_ms: 120000
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

/// Poll `GET /workflow-runs/{id}` until it surfaces a pending elicitation,
/// returning the `elicitation_id`. Panics on early terminal status / timeout.
async fn wait_for_pending_elicit(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
    let deadline = Instant::now() + Duration::from_secs(20);
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
        if let Some(id) = run["pending_elicitation_json"]["elicitation_id"].as_str() {
            return Uuid::parse_str(id).expect("elicitation_id uuid");
        }
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "completed" | "failed" | "cancelled") {
            panic!("run {run_id} reached '{status}' before pausing on elicit: {run}");
        }
        if Instant::now() >= deadline {
            panic!("run {run_id} never paused on elicit within 20s: {run}");
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

#[tokio::test]
async fn conversation_bound_midrun_elicit_resumes_and_completes() {
    let server = plain_server().await;
    let user = workflow_user(&server, "conv_elicit").await;

    // CONVERSATION-BOUND: a stub model + a conversation bound to it. The run is
    // started with `conversation_id` (NOT `model_id`), so the runner resolves
    // its model from the conversation — the path real_stack exercises only
    // under a real LLM. KEEP `_stub` alive for the run's duration.
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let wf = import_dev_workflow(
        &server,
        &user.token,
        "conv-midrun-elicit",
        CONV_ELICIT_WORKFLOW_YAML,
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id");

    // Mocks bypass the two `llm` steps (no API key); the `gate` is NOT mocked,
    // so it really dispatches + pauses mid-run.
    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "mid-run elicit" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "a": { "marker": "A_RAN" }, "b": "B_RAN" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // ── The run is genuinely bound to the conversation (not standalone). ──
    let db = db_pool(&server).await;
    let bound: (Uuid, Option<Uuid>) =
        sqlx::query_as("SELECT id, conversation_id FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("read run row");
    assert_eq!(
        bound.1,
        Some(conv_id),
        "the run must be bound to the conversation (conversation_id set)"
    );

    // ── Pauses MID-RUN on the gate: `a` has run, `gate`/`b` have not. ──
    let elicitation_id = wait_for_pending_elicit(&server, &user.token, run_id).await;
    let parked: Json =
        sqlx::query_scalar::<_, Json>("SELECT to_jsonb(r) FROM workflow_runs r WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("read parked run");
    assert!(
        parked["step_outputs_json"].get("a").is_some(),
        "the pre-gate step `a` ran before the pause: {parked}"
    );
    assert!(
        parked["step_outputs_json"].get("b").is_none(),
        "the post-gate step `b` has no output while paused: {parked}"
    );

    // ── Answer the elicit mid-run; the DAG continues. ──
    let ack_resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "response": { "approved": true } }))
        .send()
        .await
        .expect("submit elicit");
    assert_eq!(ack_resp.status(), 200, "mid-run elicit submit must 200");

    // ── Completes after the answer, with the POST-gate step reflected. ──
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "conversation-bound run resumes past the mid-run elicit and completes: {final_run}"
    );
    assert_eq!(
        final_run["final_output_json"]["result_a"]["value_preview"], "A_RAN",
        "pre-gate step `a` output is carried through: {final_run}"
    );
    assert_eq!(
        final_run["final_output_json"]["result_b"]["value_preview"], "B_RAN",
        "post-gate step `b` MUST run after the elicit is answered (DAG not truncated): {final_run}"
    );
    assert_eq!(
        final_run["final_output_json"]["gate_resp"]["value_preview"], "true",
        "the gate resolves with the submitted mid-run response: {final_run}"
    );
    for step in ["a", "gate", "b"] {
        assert!(
            final_run["step_outputs_json"].get(step).is_some(),
            "step `{step}` has a persisted output after completion: {final_run}"
        );
    }

    db.close().await;
}
