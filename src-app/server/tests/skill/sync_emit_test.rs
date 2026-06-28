//! Realtime-sync emission coverage for the skill entities.
//!
//! Proves a real REST mutation through the production handler emits the right
//! `sync` frame to the right audience, end-to-end (handler → `sync_publish` →
//! registry → SSE), via `SyncProbe`. `SyncEntity` serializes `snake_case`, so
//! the wire strings are `skill` (user/dual-audience) and `skill_system`
//! (admin-only). Mirrors `tests/mcp/sync_emit_test.rs`.
//!
//! - `skill` (OWNER): a user deleting their OWN user-scope skill sees a
//!   `skill`/`delete` frame; an unrelated user stays silent
//!   (`handlers::delete_user_skill` → `emit_user_skill`).
//! - `skill_system` (PERMISSION `skills::manage_system`) + `skill`
//!   (PERMISSION `skills::read`): a system-skill update is DUAL-AUDIENCE —
//!   `handlers::update_system_skill` → `emit_system_skill` emits BOTH frames.
//!   The admin observes `skill_system`; a separate `skills::read` holder
//!   observes `skill`; a bystander with neither stays silent.

use std::time::Duration;

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog,
};
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{
    create_user_with_only_permissions, create_user_with_permissions,
};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

// =====================================================
// skill — OWNER audience (user-scope delete)
// =====================================================

#[tokio::test]
async fn user_skill_delete_is_delivered_to_owner_not_other_users() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;

    // Install a user-scope skill owned by `admin`.
    let body = install_fixture_skill(&server, &admin.token).await;
    let skill_id = body["skill"]["id"].as_str().expect("skill id").to_string();

    // An unrelated user (default group → profile::read) can subscribe but must
    // never see the owner-scoped frame.
    let other = create_user_with_permissions(&server, "skill_sync_other", &[]).await;

    let mut owner_probe = SyncProbe::open(&server, &admin.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    // Delete through the production handler — emits emit_user_skill(Delete).
    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/skills/{skill_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete skill");
    assert!(
        resp.status().is_success(),
        "user skill delete should succeed, got {}",
        resp.status()
    );

    let frame = owner_probe
        .expect_event("skill", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, skill_id, "frame carries the deleted skill's id");

    other_probe.expect_silence(SILENCE_WINDOW).await;
}

// =====================================================
// skill_system + skill — DUAL-AUDIENCE (system-scope update)
// =====================================================

#[tokio::test]
async fn system_skill_update_delivers_to_manage_system_and_read_holders_only() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;

    // Install a system-scope skill.
    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/system/install-from-hub"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("system install");
    assert_eq!(resp.status(), 201, "system install should 201");
    let body: Json = resp.json().await.expect("parse install body");
    let skill_id = body["skill"]["id"].as_str().expect("skill id").to_string();

    // A user holding ONLY skills::read (+ profile::read to subscribe): receives
    // the dual-audience `skill` frame, never `skill_system`.
    let reader = create_user_with_only_permissions(
        &server,
        "skill_sync_reader",
        &["skills::read", "profile::read"],
    )
    .await;
    // A bystander with neither read nor manage_system: silent on both.
    let bystander =
        create_user_with_only_permissions(&server, "skill_sync_bystander", &["profile::read"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut reader_probe = SyncProbe::open(&server, &reader.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    // Update through the production handler — emits emit_system_skill(Update),
    // which publishes BOTH `skill_system` (manage_system) and `skill` (read).
    let upd = reqwest::Client::new()
        .put(server.api_url(&format!("/skills/system/{skill_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "display_name": "Renamed System Skill" }))
        .send()
        .await
        .expect("update system skill");
    assert_eq!(upd.status(), 200, "system skill update should be 200");

    // Admin (skills::manage_system) observes skill_system.
    let admin_frame = admin_probe
        .expect_event("skill_system", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(admin_frame.id, skill_id);

    // A separate skills::read holder observes the dual-audience `skill` frame.
    let reader_frame = reader_probe
        .expect_event("skill", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(reader_frame.id, skill_id);
    assert!(Uuid::parse_str(&reader_frame.id).is_ok());

    // A user lacking both perms stays silent.
    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}
