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
}
