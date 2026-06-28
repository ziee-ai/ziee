//! Realtime-sync emission for the `workflow_run` entity.
//!
//! `SyncEntity::WorkflowRun` is owner-scoped (events.rs::emit_workflow_run uses
//! `Audience::owner`). The runner fires `workflow_run` lifecycle frames as a run
//! progresses; this asserts, over the REAL path (runner → publish → registry →
//! SSE), that the run's OWNER receives a `workflow_run` frame carrying the
//! run id, and a different user does NOT.

use std::time::Duration;

use serde_json::json;

use super::{
    FIXTURE_WORKFLOW_YAML, import_dev_workflow, plain_server, run_workflow, stub_conversation,
    workflow_user,
};
use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(20);
const SILENCE_WINDOW: Duration = Duration::from_secs(2);

#[tokio::test]
async fn workflow_run_lifecycle_emits_owner_scoped_sync() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_sync_owner").await;
    let other = workflow_user(&server, "wf_sync_other").await;

    let wf = import_dev_workflow(
        &server,
        &owner.token,
        "research-summarize-write",
        FIXTURE_WORKFLOW_YAML,
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let (_stub, conv_id) = stub_conversation(&server, &owner.user_id, &owner.token).await;

    // Subscribe BEFORE running so no lifecycle frame is missed.
    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let run = run_workflow(
        &server,
        &owner.token,
        &wf_id,
        json!({
            "inputs": { "topic": "quantum entanglement" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                "research": [{"title": "Mock A", "url": "https://example.com/a"}],
                "summarize": ["a correlation between particles"],
                "write": "MEMO from a mocked run"
            }
        }),
    )
    .await;
    let run_id = run["run_id"].as_str().expect("run_id").to_string();

    // The runner emits `workflow_run`/update lifecycle frames; the owner sees one
    // carrying the run id.
    let frame = owner_probe
        .expect_event("workflow_run", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, run_id,
        "the workflow_run frame must carry the run id"
    );

    // A different user (not the run owner) must never observe it.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}
