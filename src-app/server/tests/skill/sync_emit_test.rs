//! Realtime-sync emission for the `Skill` / `SkillSystem` entities.
//!
//! A user install-from-hub emits `Skill`/create OWNER-scoped (events.rs
//! emit_user_skill); a system install emits BOTH `SkillSystem` (to
//! skills::manage_system holders) AND `Skill` (to skills::read holders)
//! (emit_system_skill). Asserted over the REAL path (handler → publish →
//! registry → SSE) via SyncProbe.

use std::time::Duration;

use serde_json::json;

use super::{admin_and_refresh, install_fixture_skill, FIXTURE_SKILL_NAME, server_with_skill_catalog};
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
const SILENCE: Duration = Duration::from_secs(2);

#[tokio::test]
async fn user_skill_install_emits_owner_scoped_skill_entity() {
    let (server, _mock) = server_with_skill_catalog().await;
    // admin refreshes the mock catalog + installs (becomes the owner).
    let admin = admin_and_refresh(&server).await;
    let other = create_user_with_permissions(&server, "skill_sync_other", &["skills::read"]).await;

    let mut owner_probe = SyncProbe::open(&server, &admin.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let body = install_fixture_skill(&server, &admin.token).await;
    let skill_id = body["skill"]["id"].as_str().expect("skill id").to_string();

    let frame = owner_probe
        .expect_event("skill", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, skill_id, "skill frame carries the new skill id");

    // Owner-scoped: a different user must never observe another user's install.
    other_probe.expect_silence(SILENCE).await;
}

#[tokio::test]
async fn system_skill_install_emits_skill_system_and_skill() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    // A separate observer holding BOTH audience perms.
    let observer = create_user_with_permissions(
        &server,
        "skill_sync_observer",
        &["skills::manage_system", "skills::read"],
    )
    .await;

    let mut observer_probe = SyncProbe::open(&server, &observer.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/system/install-from-hub"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("system install");
    assert_eq!(resp.status(), 201, "system install should 201");
    let body: serde_json::Value = resp.json().await.unwrap();
    let skill_id = body["skill"]["id"].as_str().expect("skill id").to_string();

    // emit_system_skill fires BOTH entities (order not guaranteed).
    let f1 = observer_probe
        .expect_event_any(&["skill_system", "skill"], "create", EVENT_TIMEOUT)
        .await;
    let f2 = observer_probe
        .expect_event_any(&["skill_system", "skill"], "create", EVENT_TIMEOUT)
        .await;
    let entities: std::collections::HashSet<&str> =
        [f1.entity.as_str(), f2.entity.as_str()].into_iter().collect();
    assert!(entities.contains("skill_system"), "must emit skill_system: {entities:?}");
    assert!(entities.contains("skill"), "must also emit skill: {entities:?}");
    assert_eq!(f1.id, skill_id);
    assert_eq!(f2.id, skill_id);
}
