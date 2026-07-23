//! TEST-15 / TEST-16 — the workflow `kind: agent` step host (ITEM-18..23): a
//! workflow with a single `kind: agent` step runs the shared `agent-core` loop
//! against the run's (stub) model and completes, recording a step output. This is
//! the workflow half of the shared-loop migration — the same `AgentCore` + ports
//! that back chat, driven by the workflow runner.

use serde_json::json;
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user};

const AGENT_YAML: &str = r#"inputs:
  - name: topic
    required: true
steps:
  - id: think
    kind: agent
    prompt: "Give a one-line greeting about {{ inputs.topic }}."
outputs:
  - name: result
    from: "{{ think.output }}"
"#;

#[tokio::test]
async fn workflow_agent_step_runs_the_shared_loop_to_completion() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_agent_user").await;

    let wf = import_dev_workflow(&server, &user.token, "agent-step", AGENT_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // The stub model backs the run's conversation → the agent loop calls it,
    // gets a canned text answer (no tool call), and the step finalizes.
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "scheduling" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;

    assert_eq!(
        final_run["status"], "completed",
        "the kind:agent step should run the loop to completion; got: {final_run}"
    );

    // The agent step recorded an output.
    let outputs = &final_run["step_outputs_json"];
    assert!(
        outputs.get("think").is_some(),
        "the agent step `think` must record output metadata: {final_run}"
    );
}
