//! D2 (`data:` seeding) + E5 (strict full-jsonschema submit validation) for the
//! `kind: elicit` step.
//!
//! Plan Part-B matrix item 7:
//!   - a run with an elicit step seeded `data: "{{ prev.output }}"` → the
//!     `pending_elicitation_json` (and the SSE snapshot frame) carry the seeded
//!     array, with native JSON types preserved (whole-value ref → real array);
//!   - submitting a SCHEMA-VALID response → 200 + the step output is the
//!     response;
//!   - submitting a response that violates the schema (enum / type / required)
//!     → 422 `WORKFLOW_ELICIT_SCHEMA_MISMATCH` (full jsonschema, E5).
//!
//! The seed source is a mock-short-circuited `llm` step (no tokens): a
//! `{{ screen.output }}` whole-value ref resolves to the canned array via the
//! same type-preserving renderer the `tool` step uses.

use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user};
use crate::common::TestServer;

/// A 2-step workflow: an `llm` `screen` step (mock-short-circuited to a canned
/// array-of-objects) → an `elicit` `review` step whose `data:` seeds from
/// `{{ screen.output }}` and whose schema requires an array of `{include:bool}`
/// rows under `rows`. The elicit output is the submitted response.
const SEEDED_ELICIT_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: screen
    kind: llm
    prompt: "screen {{ inputs.topic }}"
    output_format: json
  - id: review
    kind: elicit
    message: "Review the screened rows"
    data: "{{ screen.output }}"
    schema:
      type: object
      properties:
        rows:
          type: array
          items:
            type: object
            properties:
              title: { type: string }
              include: { type: boolean }
            required: [include]
      required: [rows]
    timeout_ms: 300000
    depends_on: [screen]
outputs:
  - name: decision
    from: "{{ review.output }}"
    expose: full
"#;

/// The canned screening output the `llm` mock returns — an array of objects.
/// This is what the elicit `data:` seed must carry through verbatim (typed).
fn screened_rows() -> Json {
    json!([
        { "title": "Paper A", "include": true },
        { "title": "Paper B", "include": false }
    ])
}

/// Poll the run row until `pending_elicitation_json` is set; return the full
/// pending object (it carries `elicitation_id`, `schema`, and the seeded
/// `data`). Panics on timeout / premature terminal.
async fn poll_pending(server: &TestServer, token: &str, run_id: Uuid) -> Json {
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
            return Json::Object(pending.clone());
        }
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "failed" | "cancelled" | "completed") {
            panic!("run {run_id} terminated '{status}' before pausing on elicit: {run}");
        }
        if Instant::now() >= deadline {
            panic!("run {run_id} never set pending_elicitation_json: {run}");
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

/// Start a seeded elicit run, returning `(token, run_id, pending)`.
async fn start_seeded_run(server: &TestServer, slug: &str) -> (String, Uuid, Json) {
    let user = workflow_user(server, &format!("elseed_{slug}")).await;
    let wf = import_dev_workflow(server, &user.token, slug, SEEDED_ELICIT_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    // A conversation supplies the model snapshot (no token spent — both the llm
    // step is mocked and the elicit step never dispatches to a provider).
    let (_stub, conv_id) = stub_conversation(server, &user.user_id, &user.token).await;

    let run = run_workflow(
        server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "x" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "screen": screened_rows() }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let pending = poll_pending(server, &user.token, run_id).await;
    (user.token, run_id, pending)
}

#[tokio::test]
async fn elicit_data_seed_carries_prior_step_array_typed() {
    // D2: the pending record's `data` is the prior step's whole-value output,
    // preserved as a real JSON array (not stringified).
    let server = plain_server().await;
    let (token, run_id, pending) = start_seeded_run(&server, "seed-typed").await;

    let data = &pending["data"];
    assert!(
        data.is_array(),
        "seeded `data` must be a real JSON array, not a string: {pending}"
    );
    assert_eq!(
        data, &screened_rows(),
        "seeded `data` equals the prior step's output verbatim: {pending}"
    );
    // Native types preserved inside the rows (booleans stay booleans).
    assert_eq!(data[0]["include"], json!(true));
    assert_eq!(data[1]["include"], json!(false));

    // The SSE snapshot frame (replayed on connect to a paused run) also carries
    // the seeded data via `pending_elicitation_json`.
    let snapshot_data = read_sse_snapshot_pending_data(&server, &token, run_id).await;
    assert_eq!(
        snapshot_data,
        Some(screened_rows()),
        "the SSE snapshot frame carries the seeded array"
    );
}

/// Subscribe to `/workflow-runs/{id}/events` and, with a bounded read, return
/// the `pending_elicitation_json.data` from the replayed `snapshot` frame. The
/// run is paused on elicit, so the stream stays open after the
/// connected+snapshot frames — we read only the first chunks under a timeout
/// (the same `resp.chunk()`-in-a-timeout pattern the code_sandbox SSE tests use).
async fn read_sse_snapshot_pending_data(
    server: &TestServer,
    token: &str,
    run_id: Uuid,
) -> Option<Json> {
    let mut resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/events")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("subscribe events");
    assert_eq!(resp.status(), 200, "events subscribe should 200");

    // The connected + snapshot frames are emitted immediately on connect; read
    // chunks for up to ~3s looking for the pending payload.
    let mut acc = String::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), resp.chunk()).await {
            Ok(Ok(Some(bytes))) => {
                acc.push_str(&String::from_utf8_lossy(&bytes));
                if acc.contains("\"pending_elicitation_json\"")
                    || acc.contains("elicitationRequired")
                {
                    break;
                }
            }
            Ok(Ok(None)) => break, // stream closed
            Ok(Err(_)) => break,
            Err(_) => {
                // Timed out waiting for the next chunk — what we have is enough.
                if !acc.is_empty() {
                    break;
                }
            }
        }
    }

    // Parse the SSE `data:` payloads and find one carrying the seeded array.
    for line in acc.lines() {
        if let Some(payload) = line.strip_prefix("data:") {
            if let Ok(v) = serde_json::from_str::<Json>(payload.trim()) {
                // snapshot frame → pending_elicitation_json.data
                if let Some(d) = v
                    .get("pending_elicitation_json")
                    .and_then(|p| p.get("data"))
                {
                    return Some(d.clone());
                }
                // elicitationRequired frame → data
                if let Some(d) = v.get("data") {
                    if d.is_array() {
                        return Some(d.clone());
                    }
                }
            }
        }
    }
    None
}

#[tokio::test]
async fn elicit_schema_valid_edited_response_completes_with_that_output() {
    // E5/D3: a schema-valid (edited) response → 200; after resume the run
    // completes and the elicit step's output IS the submitted response.
    let server = plain_server().await;
    let (token, run_id, pending) = start_seeded_run(&server, "seed-valid").await;
    let elicitation_id = pending["elicitation_id"].as_str().expect("elicitation_id");

    // The user edits the seeded rows (flips Paper B to include) and submits.
    let edited = json!({
        "rows": [
            { "title": "Paper A", "include": true },
            { "title": "Paper B", "include": true }
        ]
    });
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "response": edited }))
        .send()
        .await
        .expect("submit edited response");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse ack");
    assert_eq!(status, 200, "schema-valid submit should 200: {body}");
    assert_eq!(body["status"], "delivered", "ack reports delivered: {body}");

    // Run resumes + completes; the elicit step output is the edited response.
    let final_run = poll_run(&server, &token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run resumes + completes: {final_run}");

    let (out_status, out_body) = read_review_output(&server, &token, run_id).await;
    assert_eq!(out_status, 200, "review output readable: {out_body}");
    let out: Json = serde_json::from_str(&out_body).expect("review output JSON");
    assert_eq!(
        out["rows"][1]["include"],
        json!(true),
        "the elicit step output is the SUBMITTED (edited) response: {out}"
    );
}

/// Read the `review` (elicit) step output via the per-step endpoint.
async fn read_review_output(
    server: &TestServer,
    token: &str,
    run_id: Uuid,
) -> (reqwest::StatusCode, String) {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/review")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("read review output");
    let status = resp.status();
    (status, resp.text().await.unwrap_or_default())
}

#[tokio::test]
async fn elicit_schema_violating_response_is_422() {
    // E5: a response that violates the schema (a row's `include` is a string,
    // not a boolean — a NESTED array-item type error full jsonschema catches)
    // → 422 WORKFLOW_ELICIT_SCHEMA_MISMATCH.
    let server = plain_server().await;
    let (token, run_id, pending) = start_seeded_run(&server, "seed-422").await;
    let elicitation_id = pending["elicitation_id"].as_str().expect("elicitation_id");

    let bad = json!({
        "rows": [
            { "title": "Paper A", "include": "yes" }
        ]
    });
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "response": bad }))
        .send()
        .await
        .expect("submit bad response");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 422, "nested type violation must 422: {body}");
    assert!(
        body.contains("WORKFLOW_ELICIT_SCHEMA_MISMATCH"),
        "code surfaced: {body}"
    );
}

#[tokio::test]
async fn elicit_missing_required_field_is_422() {
    // E5: omitting a required top-level key (`rows`) → 422.
    let server = plain_server().await;
    let (token, run_id, pending) = start_seeded_run(&server, "seed-req-422").await;
    let elicitation_id = pending["elicitation_id"].as_str().expect("elicitation_id");

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "response": { "not_rows": [] } }))
        .send()
        .await
        .expect("submit missing-required");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 422, "missing required `rows` must 422: {body}");
    assert!(
        body.contains("WORKFLOW_ELICIT_SCHEMA_MISMATCH"),
        "code surfaced: {body}"
    );
}
