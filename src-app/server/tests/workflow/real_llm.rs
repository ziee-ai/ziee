//! Real-LLM, multi-kind workflow run. Drives a COMPLEX workflow end-to-end
//! against a REAL provider (Groq-first; see `get_or_create_groq_first_model`) —
//! no mocks for the llm steps — exercising `llm` + `llm_map` (fan-out) +
//! `elicit` (answered mid-run) + a downstream `llm` in a single DAG, and
//! asserts it actually completes with real model output.
//!
//! NO soft-skip: the Groq-first helper PANICS if no provider key is set
//! (`feedback_no_ignore_unless_platform` — `tests/.env.test` ships working LLM
//! keys, so a real-LLM test must RUN, not silently pass). Prefer Groq
//! (`llama-3.3-70b-versatile`) — cheap + tool-capable; falls back to
//! ANTHROPIC/OPENAI/GEMINI only when no Groq key is present.
//!
//! Sandbox (`kind: sandbox`) steps are covered separately in a rootfs-gated
//! test (`real_stack.rs` combines all kinds incl. sandbox) — they need a
//! code_sandbox rootfs that isn't in the default suite.

use std::time::Duration;

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{import_dev_workflow, poll_run, run_workflow, workflow_user};
use crate::common::TestServer;

/// llm (json array) → llm_map (fan-out) → elicit (mid-run) → llm (synthesis).
/// Prompts are tight + the fan-out is 2 items to keep token spend tiny.
const COMPLEX_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: list_aspects
    kind: llm
    prompt: |
      List exactly 2 short aspects of "{{ inputs.topic }}" as a JSON array
      of strings. Return ONLY the JSON array, no prose.
    output_format: json
  - id: describe_each
    kind: llm_map
    for_each: "{{ list_aspects.output }}"
    item_var: aspect
    prompt: |
      In ONE short sentence, describe the aspect "{{ aspect }}" of
      {{ inputs.topic }}.
    max_parallel: 2
    on_error: skip
    depends_on: [list_aspects]
  - id: confirm
    kind: elicit
    message: "Proceed to synthesize {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        proceed:
          type: boolean
      required: [proceed]
    timeout_ms: 120000
    depends_on: [describe_each]
  - id: synthesize
    kind: llm
    prompt: |
      Write a 2-sentence summary about "{{ inputs.topic }}" using these
      descriptions: {{ describe_each.output | json }}.
      The user's proceed flag was {{ confirm.output.proceed }}.
    depends_on: [confirm]
outputs:
  - name: summary
    from: "{{ synthesize.output }}"
    expose: full
"#;

/// Poll GET /workflow-runs/{id} until `pending_elicitation_json` is set,
/// returning the elicitation_id. Panics if the run terminates first.
async fn wait_for_elicitation(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
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
        if let Some(p) = run["pending_elicitation_json"].as_object() {
            if let Some(id) = p.get("elicitation_id").and_then(|v| v.as_str()) {
                return Uuid::parse_str(id).expect("elicitation_id uuid");
            }
        }
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "completed" | "failed" | "cancelled") {
            panic!("run {run_id} reached '{status}' before pausing on elicit: {run}");
        }
        if std::time::Instant::now() >= deadline {
            panic!("run {run_id} never paused on elicit within 120s: {run}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn real_llm_complex_workflow_llm_map_and_elicit_completes() {
    let server = TestServer::start().await;
    let user = workflow_user(&server, "wf_real_llm_user").await;

    // Real provider + model — Groq-first (cheap, tool-capable
    // `llama-3.3-70b-versatile`), granted to the user, then a conversation
    // bound to it (the run snapshots its model). PANICS if no provider key is
    // set (NO soft-skip): `tests/.env.test` ships keys, so this must RUN.
    let model = crate::chat::helpers::get_or_create_groq_first_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().expect("model id")).expect("uuid");
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("real-llm workflow"),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("conv id").to_string();

    // Dev-import the complex workflow (no mocks — real llm steps run).
    let wf = import_dev_workflow(&server, &user.token, "real-complex", COMPLEX_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // Kick the run (real provider; NO mocks).
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "espresso coffee" },
            "conversation_id": conv_id,
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // The DAG runs list_aspects (real llm) → describe_each (real llm_map)
    // → pauses on confirm (elicit). Answer it so the run resumes.
    let elicitation_id = wait_for_elicitation(&server, &user.token, run_id).await;
    let ack = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "response": { "proceed": true } }))
        .send()
        .await
        .expect("submit elicit");
    assert_eq!(ack.status(), 200, "elicit submit should 200");

    // Now synthesize (real llm) runs and the run completes.
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "real-LLM complex run should complete; got: {final_run}"
    );

    // Per-step output metadata for all four steps (llm + llm_map + elicit + llm).
    let outputs = &final_run["step_outputs_json"];
    for step in ["list_aspects", "describe_each", "confirm", "synthesize"] {
        assert!(
            outputs.get(step).is_some(),
            "step '{step}' recorded output metadata: {outputs}"
        );
    }

    // The declared `summary` output resolved from the real synthesis.
    let final_output = &final_run["final_output_json"];
    assert!(
        final_output.get("summary").is_some(),
        "final_output carries the declared `summary`: {final_run}"
    );

    // Fetch the real synthesized text and sanity-check it's non-trivial.
    let summary_resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/synthesize")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read synthesize output");
    assert_eq!(summary_resp.status(), 200, "synthesize output endpoint 200");
    let summary = summary_resp.text().await.expect("summary text");
    assert!(
        summary.trim().len() > 20,
        "real LLM produced a non-trivial summary; got: {summary:?}"
    );
}
