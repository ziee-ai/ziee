//! Realtime-sync emission for the `workflow` entity (gap 2b4d98f76c40).
//!
//! A user-scope dev import emits `SyncEntity::Workflow`/`Create` to the OWNER
//! (`events::emit_user_workflow`, owner-scoped). Asserts the importing user's
//! sync stream observes the frame carrying the new workflow id, and a second
//! user never sees it. The other workflow sync variants
//! (`WorkflowSystem`/`WorkflowRun`) ride the same `events.rs` `sync_publish`
//! path with different audiences.

use std::time::Duration;

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

use super::import_dev_workflow;

const MINIMAL_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: full
"#;

#[tokio::test]
async fn user_workflow_import_emits_sync_to_owner_only() {
    let server = TestServer::start().await;
    let owner = create_user_with_permissions(
        &server,
        "wf_sync_owner",
        &["workflows::read", "workflows::install"],
    )
    .await;
    // A second user — baseline subscriber, must never see the owner's workflow.
    let other = create_user_with_permissions(&server, "wf_sync_other", &[]).await;
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
    system_import_workflow, workflow_user,
};
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

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

#[tokio::test]
async fn user_workflow_import_emits_owner_scoped_workflow_entity() {
    // emit_user_workflow(Create) → SyncEntity::Workflow, owner-scoped. Open the
    // probes BEFORE the import so the create frame isn't missed.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_def_owner").await;
    let other = workflow_user(&server, "wf_def_other").await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let body = import_dev_workflow(&server, &owner.token, "sync-wf", MINIMAL_WORKFLOW_YAML).await;
    let workflow_id = body["id"].as_str().expect("import returns the workflow id").to_string();

    let frame = owner_probe
        .expect_event("workflow", "create", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, workflow_id, "frame carries the new workflow id");

    // Owner-scoped: an unrelated user observes nothing.
    other_probe.expect_silence(Duration::from_secs(1)).await;
    let wf = import_dev_workflow(
        &server,
        &owner.token,
        "owner-scoped-def",
        FIXTURE_WORKFLOW_YAML,
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let frame = owner_probe
        .expect_event("workflow", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, wf_id, "the workflow frame must carry the workflow id");

    // Owner-scoped: a different user must never observe another user's workflow.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn system_workflow_import_emits_workflow_system_and_workflow() {
    // emit_system_workflow(Create) → BOTH SyncEntity::WorkflowSystem (to
    // workflows::manage_system holders) AND SyncEntity::Workflow (to
    // workflows::read holders). A second admin connection (not the importer's
    // REST request) observes both frames.
    let server = plain_server().await;
    let importer = create_user_with_permissions(
        &server,
        "wf_sys_importer",
        &["workflows::manage_system", "workflows::read"],
    )
    .await;
    let observer = create_user_with_permissions(
        &server,
        "wf_sys_observer",
        &["workflows::manage_system", "workflows::read"],
    )
    .await;

    let mut observer_probe = SyncProbe::open(&server, &observer.token).await;

    let wf = system_import_workflow(
        &server,
        &importer.token,
        "system-scoped-def",
        FIXTURE_WORKFLOW_YAML,
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // Both entities fire on the same mutation (order not guaranteed); collect
    // two frames and assert both entities appear, each carrying the id.
    let f1 = observer_probe
        .expect_event_any(&["workflow_system", "workflow"], "create", EVENT_TIMEOUT)
        .await;
    let f2 = observer_probe
        .expect_event_any(&["workflow_system", "workflow"], "create", EVENT_TIMEOUT)
        .await;
    let entities: std::collections::HashSet<&str> =
        [f1.entity.as_str(), f2.entity.as_str()].into_iter().collect();
    assert!(
        entities.contains("workflow_system"),
        "system import must emit workflow_system: {entities:?}"
    );
    assert!(
        entities.contains("workflow"),
        "system import must also emit workflow: {entities:?}"
    );
    assert_eq!(f1.id, wf_id);
    assert_eq!(f2.id, wf_id);
}
