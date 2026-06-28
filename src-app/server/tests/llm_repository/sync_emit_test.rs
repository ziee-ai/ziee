//! Realtime-sync emission for the `llm_repository` entity.
//!
//! The LLM-repository surface is permission-scoped: a mutation fans out only
//! to connections whose snapshot satisfies `llm_repositories::read` (admins
//! always qualify). These tests assert, over the REAL path (handler → publish
//! → registry → SSE), that an admin creating/updating a repository produces an
//! `llm_repository`/<action> frame carrying the row id, and that a user
//! lacking the read perm never observes it.

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// POST /llm-repositories as `token`, returning the new repository id.
async fn create_repository(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "url": "https://example.com/test",
            "auth_type": "api_key",
            "auth_config": { "api_key": "test-api-key-12345" },
            "enabled": true
        }))
        .send()
        .await
        .expect("create repository request failed");
    assert_eq!(res.status(), 201, "repository create should return 201");
    let row: serde_json::Value = res.json().await.unwrap();
    row["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn admin_create_delivers_llm_repository_event_other_user_silent() {
    let server = crate::common::TestServer::start().await;
    // Actor holds the endpoint's manage perm (create) + the audience read perm.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_repo_admin",
        &["llm_repositories::create", "llm_repositories::read"],
    )
    .await;
    // Bob holds only the baseline (default group → profile::read); enough to
    // subscribe, but he lacks `llm_repositories::read` so he must stay silent.
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_repo_bob",
        &[],
    )
    .await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let id = create_repository(&server, &admin.token, "Sync Repo Create").await;

    let frame = admin_probe
        .expect_event("llm_repository", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new repository's id");

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn admin_update_delivers_llm_repository_event_other_user_silent() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_repo_upd_admin",
        &[
            "llm_repositories::create",
            "llm_repositories::edit",
            "llm_repositories::read",
        ],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_repo_upd_bob",
        &[],
    )
    .await;

    let id = create_repository(&server, &admin.token, "Sync Repo Update").await;

    // Subscribe AFTER the create so we observe only the update frame.
    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "name": "Sync Repo Update Renamed" }))
        .send()
        .await
        .expect("update repository request failed");
    assert_eq!(res.status(), 200, "repository update should return 200");

    let frame = admin_probe
        .expect_event("llm_repository", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the updated repository's id");

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}

/// Sync entity permission via GROUP MEMBERSHIP (not direct grant). The existing
/// tests grant llm_repositories::read DIRECTLY; this proves the sync audience
/// honors a read perm a user holds ONLY through group membership — i.e. a
/// permission change via group assignment puts the user into the entity's
/// audience.
#[tokio::test]
async fn group_derived_read_perm_puts_user_in_llm_repository_audience() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Admin can create groups, assign users, and create repositories.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "grp_repo_admin",
        &[
            "groups::create",
            "groups::assign_users",
            "llm_repositories::create",
        ],
    )
    .await;
    // Bob holds NO direct llm_repositories::read — only the default baseline.
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "grp_repo_bob",
        &[],
    )
    .await;

    // A group that GRANTS llm_repositories::read, then add Bob to it so his
    // effective perms include the read VIA the group.
    let group: serde_json::Value = client
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("repo-read-group-{}", uuid::Uuid::new_v4()),
            "description": "grants llm_repositories::read",
            "permissions": ["llm_repositories::read"]
        }))
        .send()
        .await
        .expect("create group")
        .json()
        .await
        .expect("parse group");
    let group_id = group["id"].as_str().expect("group id").to_string();

    let assign = client
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": bob.user_id, "group_id": group_id }))
        .send()
        .await
        .expect("assign bob");
    assert_eq!(assign.status(), 204, "assign should 204");

    // Subscribe AFTER the assign — Bob's effective perms now include the
    // group-derived llm_repositories::read.
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    // Admin creates a repository → Bob (group-derived read) is in the audience.
    let repo_id = create_repository(&server, &admin.token, "Group Read Repo").await;
    let event = bob_probe
        .expect_event("llm_repository", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        event.id, repo_id,
        "a group-derived llm_repositories::read must place Bob in the llm_repository/create audience"
    );
}
