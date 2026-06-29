//! audit id all-75e02bfb7833 — cost/token tracking on the workflow RUN row.
//! `workflow_runs.total_tokens` is accumulated from each LLM step's response
//! `usage` (dispatch.rs:245 → repository.rs:498) and surfaced on the run model
//! (models.rs:146). Nothing asserted it lands on the row. Here a real (stub-LLM)
//! run — NO `mocks`, so the runner dispatches to the stub provider for real —
//! must finish with a non-zero `total_tokens`. The stub engine returns
//! `usage.total_tokens`, so the count is the only thing canned; the
//! accumulate-and-persist path runs for real.

use serde_json::json;
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user};

// A single plain-text llm step (no output_format) so the stub's text reply
// satisfies the workflow and the run reaches `completed`.
const ONE_STEP_LLM_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: write
    kind: llm
    prompt: "Write a sentence about {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ write.output }}"
"#;

#[tokio::test]
async fn workflow_run_records_total_tokens_from_llm_usage() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_cost_user").await;
    let wf = import_dev_workflow(&server, &user.token, "cost-track", ONE_STEP_LLM_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // Stub model + conversation → the runner snapshots a model and dispatches
    // the llm step against the stub (which returns a usage block).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "quantum entanglement" },
            "conversation_id": conv_id.to_string(),
            // No `mocks` → REAL dispatch to the stub provider.
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "stub-driven run should complete: {final_run}"
    );

    let total_tokens = final_run["total_tokens"]
        .as_u64()
        .unwrap_or_else(|| panic!("run row must carry total_tokens: {final_run}"));
    assert!(
        total_tokens > 0,
        "an executed LLM step must accumulate total_tokens on the run row, got {total_tokens}"
    );
}
