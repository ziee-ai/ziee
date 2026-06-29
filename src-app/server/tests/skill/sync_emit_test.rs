use std::time::Duration;
use super::FIXTURE_SKILL_NAME;
use super::refresh_catalog;
use super::server_with_skill_catalog;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;
use serde_json::json;
use super::admin_and_refresh;
use super::install_fixture_skill;
use serde_json::Value as Json;
use uuid::Uuid;
use crate::common::test_helpers::create_user_with_only_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Installing a USER-scope skill from the hub emits `Skill/create` to the
/// installing owner's connections (hub::handlers::create_skill_from_hub →
/// emit_user_skill). The owner's subscribed device must observe it WITHOUT a
/// reload.
#[tokio::test]
async fn user_skill_install_emits_skill_create_to_owner() {
    let (server, _mock) = server_with_skill_catalog().await;
    let user = create_user_with_permissions(
        &server,
        "skill_sync_owner",
        &["hub::catalog::read", "hub::catalog::manage", "skills::read", "skills::install"],
    )
    .await;
    refresh_catalog(&server, &user.token).await;

    let mut probe = SyncProbe::open(&server, &user.token).await;

    // Install the fixture skill as this user (no X-Sync-Connection-Id → the
    // owner's own stream is NOT origin-suppressed).
    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/install-from-hub"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("install");
    assert_eq!(resp.status(), 201, "user skill install should 201");

    let frame = probe.expect_event("skill", "create", EVENT_TIMEOUT).await;
    assert!(!frame.id.is_empty(), "skill/create frame carries the new skill id");
}

/// Installing a SYSTEM-scope skill emits the dual fan-out
/// (hub::handlers::create_system_skill_from_hub → emit_system_skill):
///   - `SkillSystem/create` to `skills::manage_system` holders (admins), AND
///   - `Skill/create` to every `skills::read` holder (their available list).
/// We assert BOTH audiences observe their respective event on a live stream.
#[tokio::test]
async fn system_skill_install_emits_skill_system_to_admin_and_skill_to_users() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = create_user_with_permissions(
        &server,
        "skill_sync_admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage_system",
        ],
    )
    .await;
    refresh_catalog(&server, &admin.token).await;
    // A plain user holding only skills::read (granted to the default group) is
    // the positive control for the user-facing fan-out.
    let viewer = create_user_with_permissions(&server, "skill_sync_viewer", &["skills::read"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut viewer_probe = SyncProbe::open(&server, &viewer.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/system/install-from-hub"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("system install");
    assert_eq!(
        resp.status(),
        201,
        "system skill install should 201: {}",
        resp.text().await.unwrap_or_default()
    );

    // Admin sees the admin-list entity; the viewer sees the available-skills
    // entity. Both prove the previously-uncovered variants reach the wire.
    let admin_frame = admin_probe
        .expect_event("skill_system", "create", EVENT_TIMEOUT)
        .await;
    assert!(!admin_frame.id.is_empty(), "skill_system/create carries an id");

    let viewer_frame = viewer_probe.expect_event("skill", "create", EVENT_TIMEOUT).await;
    assert!(!viewer_frame.id.is_empty(), "skill/create carries an id");
}

const EVENT_TIMEOUT_v2: Duration = Duration::from_secs(10);

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

