//! Realtime-sync emission coverage for the workflow SyncEntities
//! (`Workflow` / `WorkflowRun`) — neither appeared in any `expect_event()`
//! before. Proves a real mutation through the production handler/runner emits
//! the owner-scoped frame end-to-end via `SyncProbe`.

use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

use super::{
    import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user,
};
use crate::common::sync_probe::SyncProbe;

const SIMPLE_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: s1
    kind: llm
    prompt: "do {{ inputs.topic }}"
outputs:
  - name: out
    from: "{{ s1.output }}"
    expose: full
"#;

/// Dev-importing a workflow emits an owner-scoped `workflow`/`create` frame
/// (dev.rs:319 → emit_user_workflow). The owner sees it; an unrelated user
/// stays silent.
#[tokio::test]
async fn workflow_import_emits_owner_scoped_create_frame() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_sync_owner").await;
    let other = workflow_user(&server, "wf_sync_other").await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let _wf = import_dev_workflow(&server, &owner.token, "sync-wf", SIMPLE_YAML).await;

    owner_probe
        .expect_event("workflow", "create", Duration::from_secs(5))
        .await;
    other_probe.expect_silence(Duration::from_secs(1)).await;
}

/// Running a workflow drives the runner's `workflow_run`/`update` lifecycle
/// frames to the run owner (runner.rs → emit_workflow_run). The owner's sync
/// stream observes at least one such frame as the mocked run progresses.
#[tokio::test]
async fn workflow_run_emits_owner_scoped_run_frame() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_run_sync_owner").await;
    let wf = import_dev_workflow(&server, &owner.token, "sync-run-wf", SIMPLE_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let (_stub, conv_id) = stub_conversation(&server, &owner.user_id, &owner.token).await;

    // Subscribe BEFORE the run so no lifecycle frame is missed.
    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;

    let run = run_workflow(
        &server,
        &owner.token,
        &wf_id,
        json!({
            "inputs": { "topic": "x" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "s1": "mocked output" }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // A workflow_run frame for this run reaches the owner.
    let frame = owner_probe
        .expect_event("workflow_run", "update", Duration::from_secs(10))
        .await;
    assert_eq!(frame.id, run_id.to_string(), "the run frame must carry the run id");

    // Sanity: the mocked run reaches a terminal state.
    let final_run = poll_run(&server, &owner.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "mocked run completes: {final_run}");
}
