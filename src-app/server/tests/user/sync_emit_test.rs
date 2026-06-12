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

/// When an admin edits a group's PERMISSIONS, the handler fans a `session`
/// signal out to EVERY member of the group via `publish_session_to_users` —
/// a single registry-lock acquisition that delivers Owner-scoped Session
/// frames to each member's user_id. Each member's open devices then
/// re-bootstrap /auth/me to pick up the new effective permissions
/// immediately, rather than waiting up to 60s for the per-connection
/// re-check.
///
/// The previously-existing `update_group_emits_group_update_to_groups_read_holders`
/// edits a FRESHLY-CREATED EMPTY group, so its fan-out runs over zero
/// members and proves nothing about the bulk-delivery path. This test
/// seeds two members + one uninvolved user FIRST, subscribes them, THEN
/// edits the group's permissions, and asserts:
///   - both members receive `session`/`update` with their OWN user id
///   - the uninvolved (non-member) user stays silent (Owner-scoping
///     prevents leakage even though they share the deployment)
#[tokio::test]
async fn group_permission_edit_fans_session_out_to_every_member() {
    let server = crate::common::TestServer::start().await;

    // Actor needs the perm it's trying to grant — the self-escalation
    // guard at handlers/groups.rs:172 rejects with CANNOT_GRANT_PERMISSION
    // otherwise. `users::read` is granted to the actor explicitly so it
    // can flow into the group without bypassing the guard.
    let admin = test_helpers::create_user_with_permissions(&server, "perm_edit_admin", &[
        "users::create",
        "users::read",
        "groups::create",
        "groups::edit",
        "groups::assign_users",
    ])
    .await;

    // Two members + one uninvolved user. Members are created via the admin
    // API so we need to log in as each to get a subscribable token.
    let m1 = test_helpers::create_test_user(&server, &admin.token, "permmember1", "password123")
        .await;
    let m1_id = m1["id"].as_str().expect("m1 id").to_string();
    let m1_token = login_token(&server, "permmember1", "password123").await;

    let m2 = test_helpers::create_test_user(&server, &admin.token, "permmember2", "password123")
        .await;
    let m2_id = m2["id"].as_str().expect("m2 id").to_string();
    let m2_token = login_token(&server, "permmember2", "password123").await;

    let uninvolved =
        test_helpers::create_user_with_permissions(&server, "perm_edit_uninvolved", &[]).await;

    // Create the group and assign both members BEFORE subscribing — the
    // assigns each emit `session`/`update` themselves, and we don't want
    // to consume those frames in this test (it's about the fan-out from
    // the subsequent permission edit, not from the assigns).
    let group: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("sync-perm-edit-group-{}", uuid::Uuid::new_v4()),
            "description": "sync emit test — group permission edit fan-out",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group request failed")
        .json()
        .await
        .expect("parse created group");
    let group_id = group["id"].as_str().expect("created group id").to_string();

    for (user_id, label) in [(&m1_id, "m1"), (&m2_id, "m2")] {
        let assign = reqwest::Client::new()
            .post(server.api_url("/groups/assign"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({ "user_id": user_id, "group_id": group_id }))
            .send()
            .await
            .unwrap_or_else(|e| panic!("assign {label} failed: {e}"));
        assert_eq!(
            assign.status(),
            204,
            "assign {label} should return 204"
        );
    }

    // NOW subscribe — the assigns above are already settled and won't show
    // up in any probe's frame stream.
    let mut m1_probe = SyncProbe::open(&server, &m1_token).await;
    let mut m2_probe = SyncProbe::open(&server, &m2_token).await;
    let mut uninvolved_probe = SyncProbe::open(&server, &uninvolved.token).await;

    // Edit the group's PERMISSIONS — this is the trigger for the
    // publish_session_to_users fan-out at handlers/groups.rs:225. Admin
    // holds the perms they're trying to grant (they have groups::create
    // and groups::edit themselves) so the self-escalation guard passes
    // for a benign add.
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/groups/{}", group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "permissions": ["users::read"] }))
        .send()
        .await
        .expect("group permission edit request failed");
    assert_eq!(
        res.status(),
        200,
        "admin update_group with new permissions should return 200, got {}: {}",
        res.status(),
        res.text().await.unwrap_or_default(),
    );

    // Both members receive `session`/`update` carrying THEIR OWN user_id —
    // the fan-out delivers a per-recipient Owner-scoped frame, not the
    // group's id (the Session frame.id is the affected user).
    let m1_frame = m1_probe
        .expect_event("session", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        m1_frame.id, m1_id,
        "m1's session frame.id must be m1's own user id (Owner-scoped fan-out)"
    );
    let m2_frame = m2_probe
        .expect_event("session", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        m2_frame.id, m2_id,
        "m2's session frame.id must be m2's own user id (Owner-scoped fan-out)"
    );

    // The uninvolved user (not a member) must NOT see any session frame —
    // Owner-scoping isolates each delivery to its target user. They also
    // lack groups::read so the sibling `group`/`update` audience misses
    // them too.
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
