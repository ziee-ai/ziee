//! Non-linear (diamond / fan-out + fan-in) DAG topology.
//!
//! Every other workflow run test drives a strictly SEQUENTIAL chain
//! (research → summarize → write). This proves the runner's `topo_sort_steps`
//! correctly orders a DIAMOND:
//!
//!     A ──► B ──┐
//!     │         ▼
//!     └────► C ─► D
//!
//! B and C BOTH depend on A (fan-out / parallel branches); D depends on BOTH B
//! and C (fan-in / join). All four steps are `llm` and fully MOCKED, so no live
//! provider is touched (mirrors `run_mocked.rs`). The join is asserted for real:
//! the workflow `combined` output renders a template that references BOTH branch
//! outputs, so a value of `B_RESULT+C_RESULT` proves D's render context carried
//! the outputs of both parallel branches — i.e. the diamond was scheduled and
//! joined correctly.

use serde_json::json;
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user};

const DIAMOND_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: a
    kind: llm
    prompt: "Seed for {{ inputs.topic }}"
  - id: b
    kind: llm
    prompt: "Branch B over {{ a.output }}"
    depends_on: [a]
  - id: c
    kind: llm
    prompt: "Branch C over {{ a.output }}"
    depends_on: [a]
  - id: d
    kind: llm
    prompt: "Join {{ b.output }} and {{ c.output }}"
    depends_on: [b, c]
outputs:
  - name: combined
    from: "{{ b.output }}+{{ c.output }}"
    expose: full
"#;

#[tokio::test]
async fn diamond_dag_fans_out_and_joins() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_diamond_user").await;

    let wf = import_dev_workflow(&server, &user.token, "diamond-dag", DIAMOND_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "scheduling" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                "a": "SEED",
                "b": "B_RESULT",
                "c": "C_RESULT",
                "d": "D_DONE"
            }
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;

    assert_eq!(
        final_run["status"], "completed",
        "diamond run should complete; got: {final_run}"
    );

    // All four steps of the diamond ran and recorded output metadata.
    let outputs = &final_run["step_outputs_json"];
    assert!(outputs.is_object(), "step_outputs_json is an object: {final_run}");
    for step in ["a", "b", "c", "d"] {
        assert!(
            outputs.get(step).is_some(),
            "diamond step '{step}' has output metadata: {outputs}"
        );
    }

    // The join: the `combined` output renders BOTH branch outputs. A correct
    // fan-in means D's (and the output stage's) render context carried both b
    // and c — so the rendered value is exactly "B_RESULT+C_RESULT".
    let combined = final_run["final_output_json"]["combined"].clone();
    // final_output_json stores each output as { value_preview, size_bytes,
    // expose } (see runner::resolve_outputs).
    let combined_str = combined["value_preview"].as_str().unwrap_or_default();
    assert!(
        combined_str.contains("B_RESULT") && combined_str.contains("C_RESULT"),
        "the diamond join output must carry BOTH parallel branch outputs; got combined={combined}"
    );
}
