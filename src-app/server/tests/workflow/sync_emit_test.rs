//! Realtime-sync emission coverage for the workflow entities.
//!
//! Proves a real REST mutation through the production handler emits the right
//! `sync` frame to the right audience (handler → `sync_publish` → registry →
//! SSE), via `SyncProbe`. `SyncEntity` serializes `snake_case`, so the wire
//! strings are `workflow` (user/dual-audience) and `workflow_system`
//! (admin-only). Mirrors `tests/skill/sync_emit_test.rs`. (WorkflowRun sync is
//! exercised by the E2E `13-sync/workflow-run-sync.spec.ts`.)

use std::time::Duration;

use serde_json::Value as Json;

use super::{import_dev_workflow, plain_server, system_import_workflow, workflow_user};
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{
    create_user_with_only_permissions, create_user_with_permissions,
};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

const WF_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: a
    kind: llm
    prompt: "noop"
outputs:
  - name: out
    from: "{{ a.output }}"
    expose: full
"#;

// =====================================================
// workflow — OWNER audience (user-scope delete)
// =====================================================

#[tokio::test]
async fn user_workflow_delete_is_delivered_to_owner_not_other_users() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_sync_owner").await;

    let wf = import_dev_workflow(&server, &owner.token, "sync-del", WF_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // Unrelated user (default group → profile::read): can subscribe, never sees
    // the owner-scoped frame.
    let other = create_user_with_permissions(&server, "wf_sync_other", &[]).await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflows/{wf_id}")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("delete workflow");
    assert!(
        resp.status().is_success(),
        "user workflow delete should succeed, got {}",
        resp.status()
    );

    let frame = owner_probe
        .expect_event("workflow", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, wf_id, "frame carries the deleted workflow's id");

    other_probe.expect_silence(SILENCE_WINDOW).await;
}

// =====================================================
// workflow_system + workflow — DUAL-AUDIENCE (system-scope create)
// =====================================================

#[tokio::test]
async fn system_workflow_create_delivers_to_manage_system_and_read_holders_only() {
    let server = plain_server().await;
    // Actor manages system workflows (install + manage_system) AND holds
    // workflows::read so it sits in BOTH audiences.
    let admin = create_user_with_permissions(
        &server,
        "wf_sync_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
        ],
    )
    .await;
    // A user holding ONLY workflows::read (+ profile::read to subscribe):
    // receives the dual-audience `workflow` frame, never `workflow_system`.
    let reader = create_user_with_only_permissions(
        &server,
        "wf_sync_reader",
        &["workflows::read", "profile::read"],
    )
    .await;
    // Bystander: neither read nor manage_system → silent on both.
    let bystander =
        create_user_with_only_permissions(&server, "wf_sync_bystander", &["profile::read"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut reader_probe = SyncProbe::open(&server, &reader.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    // System import → emit_system_workflow(Create): dual-audience.
    let wf: Json = system_import_workflow(&server, &admin.token, "sync-sys", WF_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let admin_frame = admin_probe
        .expect_event("workflow_system", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(admin_frame.id, wf_id);

    let reader_frame = reader_probe
        .expect_event("workflow", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(reader_frame.id, wf_id);

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}
