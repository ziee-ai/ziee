//! Realtime-sync emission tests for the user/group surface, exercised over the
//! REAL path: a real admin REST mutation produces a real `sync` frame on a
//! subscribed stream, delivered to the RIGHT audience.
//!
//! Entities covered (see `modules/sync/event.rs::audience_kind`):
//!   - `user`    → Permission("users::read")   — admin creates / updates a user.
//!   - `group`   → Permission("groups::read")  — admin creates / updates a group.
//!   - `session` → Owner(affected user)        — admin assigns / removes a user
//!                 from a group (or edits a group's permissions); the AFFECTED
//!                 user's devices re-bootstrap /auth/me.
//!   - `profile` → Owner(edited user)          — admin edits a user's profile
//!                 fields; the EDITED user's other devices re-bootstrap.
//!
//! These complement the deterministic in-source unit tests in `modules/sync/`
//! (which cover audience routing in isolation) and the generic delivery /
//! isolation / self-echo guarantees in `tests/sync/delivery_test.rs`.

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

// ============================================================================
// `user` entity — Permission("users::read")
// ============================================================================

/// An admin creating a user emits `user`/`create` to every holder of
/// `users::read`; a user lacking that permission stays silent.
#[tokio::test]
async fn create_user_emits_user_create_to_users_read_holders() {
    let server = crate::common::TestServer::start().await;

    // Actor: can create users AND read them (the audience permission).
    let actor =
        test_helpers::create_user_with_permissions(&server, "user_create_actor", &[
            "users::create",
            "users::read",
        ])
        .await;
    // Bystander: holds only the baseline default-group perms (no users::read).
    let bystander =
        test_helpers::create_user_with_permissions(&server, "user_create_bystander", &[]).await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    let created =
        test_helpers::create_test_user(&server, &actor.token, "freshuser", "password123").await;
    let created_id = created["id"].as_str().expect("created user id").to_string();

    let frame = actor_probe
        .expect_event("user", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, created_id, "frame must carry the new user's id");

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}

/// An admin updating a user's fields emits `user`/`update` to `users::read`
/// holders; a user lacking that permission stays silent.
#[tokio::test]
async fn update_user_emits_user_update_to_users_read_holders() {
    let server = crate::common::TestServer::start().await;

    // Actor needs users::create (to mint the target), users::edit (to update),
    // and users::read (the audience permission).
    let actor = test_helpers::create_user_with_permissions(&server, "user_update_actor", &[
        "users::create",
        "users::edit",
        "users::read",
    ])
    .await;
    let bystander =
        test_helpers::create_user_with_permissions(&server, "user_update_bystander", &[]).await;

    let target =
        test_helpers::create_test_user(&server, &actor.token, "updtarget", "password123").await;
    let target_id = target["id"].as_str().expect("target user id").to_string();

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/users/{}", target_id)))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({ "display_name": "Renamed By Admin" }))
        .send()
        .await
        .expect("update user request failed");
    assert_eq!(res.status(), 200, "admin update_user should return 200");

    let frame = actor_probe
        .expect_event("user", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, target_id, "frame must carry the edited user's id");

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}

// ============================================================================
// `group` entity — Permission("groups::read")
// ============================================================================

/// An admin creating a group emits `group`/`create` to every holder of
/// `groups::read`; a user lacking that permission stays silent.
#[tokio::test]
async fn create_group_emits_group_create_to_groups_read_holders() {
    let server = crate::common::TestServer::start().await;

    let actor = test_helpers::create_user_with_permissions(&server, "group_create_actor", &[
        "groups::create",
        "groups::read",
    ])
    .await;
    let bystander =
        test_helpers::create_user_with_permissions(&server, "group_create_bystander", &[]).await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    let created: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({
            "name": format!("sync-create-group-{}", uuid::Uuid::new_v4()),
            "description": "sync emit test",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group request failed")
        .json()
        .await
        .expect("parse created group");
    let created_id = created["id"].as_str().expect("created group id").to_string();

    let frame = actor_probe
        .expect_event("group", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, created_id, "frame must carry the new group's id");

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}

/// An admin updating a group emits `group`/`update` to `groups::read` holders;
/// a user lacking that permission stays silent.
#[tokio::test]
async fn update_group_emits_group_update_to_groups_read_holders() {
    let server = crate::common::TestServer::start().await;

    let actor = test_helpers::create_user_with_permissions(&server, "group_update_actor", &[
        "groups::create",
        "groups::edit",
        "groups::read",
    ])
    .await;
    let bystander =
        test_helpers::create_user_with_permissions(&server, "group_update_bystander", &[]).await;

    // Mint a non-system group to edit.
    let created: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({
            "name": format!("sync-update-group-{}", uuid::Uuid::new_v4()),
            "description": "sync emit test",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group request failed")
        .json()
        .await
        .expect("parse created group");
    let group_id = created["id"].as_str().expect("created group id").to_string();

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/groups/{}", group_id)))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({ "description": "renamed by sync emit test" }))
        .send()
        .await
        .expect("update group request failed");
    assert_eq!(res.status(), 200, "admin update_group should return 200");

    let frame = actor_probe
        .expect_event("group", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, group_id, "frame must carry the edited group's id");

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}

// ============================================================================
// `session` entity — Owner(affected user)
// ============================================================================

/// When an admin assigns a user to a group, the AFFECTED user receives a
/// `session`/`update` signal (Owner-scoped to that user) so their devices
/// re-bootstrap /auth/me. A DIFFERENT, uninvolved user must stay silent.
#[tokio::test]
async fn assign_user_to_group_emits_session_to_the_affected_user_only() {
    let server = crate::common::TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(&server, "session_admin", &[
        "users::create",
        "groups::create",
        "groups::assign_users",
    ])
    .await;

    // The affected user — they receive the `session` signal.
    let target =
        test_helpers::create_test_user(&server, &admin.token, "sessiontarget", "password123").await;
    let target_id = target["id"].as_str().expect("target user id").to_string();
    // Log in as target to obtain a token they can subscribe with.
    let target_token = login_token(&server, "sessiontarget", "password123").await;

    // An uninvolved user with no groups::read — must NOT see the session
    // (owner-scoped to target) NOR the group update (groups::read audience).
    let uninvolved =
        test_helpers::create_user_with_permissions(&server, "session_uninvolved", &[]).await;

    let group: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("sync-session-group-{}", uuid::Uuid::new_v4()),
            "description": "sync emit test",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group request failed")
        .json()
        .await
        .expect("parse created group");
    let group_id = group["id"].as_str().expect("created group id").to_string();

    let mut target_probe = SyncProbe::open(&server, &target_token).await;
    let mut uninvolved_probe = SyncProbe::open(&server, &uninvolved.token).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": target_id, "group_id": group_id }))
        .send()
        .await
        .expect("assign request failed");
    assert_eq!(res.status(), 204, "assign_user_to_group should return 204");

    let frame = target_probe
        .expect_event("session", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, target_id,
        "session frame id must be the AFFECTED user's id"
    );

    uninvolved_probe.expect_silence(SILENCE_WINDOW).await;
}

/// When an admin removes a user from a group, the AFFECTED user receives a
/// `session`/`update` signal (Owner-scoped). A different uninvolved user is
/// silent.
#[tokio::test]
async fn remove_user_from_group_emits_session_to_the_affected_user_only() {
    let server = crate::common::TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(&server, "session_rm_admin", &[
        "users::create",
        "groups::create",
        "groups::assign_users",
    ])
    .await;

    let target =
        test_helpers::create_test_user(&server, &admin.token, "sessionrmtarget", "password123")
            .await;
    let target_id = target["id"].as_str().expect("target user id").to_string();
    let target_token = login_token(&server, "sessionrmtarget", "password123").await;

    let uninvolved =
        test_helpers::create_user_with_permissions(&server, "session_rm_uninvolved", &[]).await;

    let group: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("sync-session-rm-group-{}", uuid::Uuid::new_v4()),
            "description": "sync emit test",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group request failed")
        .json()
        .await
        .expect("parse created group");
    let group_id = group["id"].as_str().expect("created group id").to_string();

    // Assign first (before subscribing, so we don't have to consume that frame).
    let assign = reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": target_id, "group_id": group_id }))
        .send()
        .await
        .expect("assign request failed");
    assert_eq!(assign.status(), 204, "assign should return 204");

    let mut target_probe = SyncProbe::open(&server, &target_token).await;
    let mut uninvolved_probe = SyncProbe::open(&server, &uninvolved.token).await;

    let res = reqwest::Client::new()
        .delete(server.api_url(&format!("/groups/{}/{}/remove", target_id, group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("remove request failed");
    assert_eq!(res.status(), 204, "remove_user_from_group should return 204");

    let frame = target_probe
        .expect_event("session", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, target_id,
        "session frame id must be the AFFECTED user's id"
    );

    uninvolved_probe.expect_silence(SILENCE_WINDOW).await;
}

// ============================================================================
// `profile` entity — Owner(edited user)
// ============================================================================

/// When an admin edits a user's profile fields, the EDITED user receives a
/// `profile`/`update` signal (Owner-scoped). Note the same admin update ALSO
/// emits `user`/`update` to users::read holders — that's fine; expect_event
/// ignores non-matching frames. A different uninvolved user stays silent.
#[tokio::test]
async fn update_user_emits_profile_to_the_edited_user_only() {
    let server = crate::common::TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(&server, "profile_admin", &[
        "users::create",
        "users::edit",
    ])
    .await;

    let target =
        test_helpers::create_test_user(&server, &admin.token, "profiletarget", "password123").await;
    let target_id = target["id"].as_str().expect("target user id").to_string();
    let target_token = login_token(&server, "profiletarget", "password123").await;

    let uninvolved =
        test_helpers::create_user_with_permissions(&server, "profile_uninvolved", &[]).await;

    let mut target_probe = SyncProbe::open(&server, &target_token).await;
    let mut uninvolved_probe = SyncProbe::open(&server, &uninvolved.token).await;

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/users/{}", target_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "display_name": "Edited By Admin" }))
        .send()
        .await
        .expect("update user request failed");
    assert_eq!(res.status(), 200, "admin update_user should return 200");

    let frame = target_probe
        .expect_event("profile", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, target_id,
        "profile frame id must be the EDITED user's id"
    );

    uninvolved_probe.expect_silence(SILENCE_WINDOW).await;
}

// ============================================================================
// Helpers
// ============================================================================

/// Log in as `username`/`password` and return the access token. Used to get a
/// subscribable token for a user created via the admin API (those users have no
/// token from creation).
async fn login_token(
    server: &crate::common::TestServer,
    username: &str,
    password: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": username, "password": password }))
        .send()
        .await
        .expect("login request failed");
    assert!(
        res.status().is_success(),
        "login for {username} should succeed, got {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.expect("parse login response");
    body["access_token"]
        .as_str()
        .expect("access_token missing from login response")
        .to_string()
}
