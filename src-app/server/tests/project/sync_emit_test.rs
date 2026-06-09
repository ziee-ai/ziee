//! Realtime-sync emission for the `project` entity.
//!
//! Asserts that a real REST mutation on `/projects` produces the correct
//! `sync` frame (`project`/`create|update|delete`) on the OWNER's subscribed
//! stream, carrying the mutated project's id — and that a DIFFERENT user who
//! does not own the project never observes the frame (owner-scoped audience).
//!
//! This exercises the full producer→registry→stream path through the real
//! handler (`handlers::{create,update,delete}_project` call `sync_publish`
//! with `SyncEntity::Project` + `Some(owner_id)` as the audience). Mirrors the
//! generic mechanism coverage in `tests/sync/delivery_test.rs`.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;

use super::helpers;
use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// The owner-scoped project permission bundle. `projects::read` is
/// Administrators-only in production, so tests grant it directly to the owner.
fn owner_project_permissions() -> &'static [&'static str] {
    &[
        "projects::create",
        "projects::read",
        "projects::edit",
        "projects::delete",
    ]
}

#[tokio::test]
async fn project_create_emits_to_owner_only() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "proj_sync_owner",
        owner_project_permissions(),
    )
    .await;
    // A second user with NO project permissions (baseline profile::read only);
    // enough to subscribe, but he must never see the owner's project event.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "proj_sync_other",
        &[],
    )
    .await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let project = helpers::create_project(&server, &owner, "Sync Project").await;
    let id = project["id"].as_str().unwrap().to_string();

    let frame = owner_probe
        .expect_event("project", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new project's id");

    // Cross-user isolation: a user who does not own the project stays silent.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn project_create_update_delete_each_emit_to_owner() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "proj_sync_cud",
        owner_project_permissions(),
    )
    .await;
    let mut probe = SyncProbe::open(&server, &owner.token).await;
    let client = reqwest::Client::new();

    // Create.
    let project = helpers::create_project(&server, &owner, "CUD Project").await;
    let id = project["id"].as_str().unwrap().to_string();
    let created = probe
        .expect_event("project", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(created.id, id);

    // Update (PUT /projects/{id} — matches the route registered in routes.rs).
    let update_resp = client
        .put(server.api_url(&format!("/projects/{}", id)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({
            "name": "CUD Project Renamed",
            "description": "updated",
            "instructions": "Speak in haiku.",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);
    let updated = probe
        .expect_event("project", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(updated.id, id);

    // Delete.
    let delete_status = helpers::delete_project(&server, &owner, &id).await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    let deleted = probe
        .expect_event("project", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(deleted.id, id);
}
