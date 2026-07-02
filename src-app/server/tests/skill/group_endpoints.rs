//! Group-centric system-skill assignment endpoints (the User Groups page
//! widget direction): `GET/PUT /api/groups/{group_id}/system-skills`.
//!
//! Mirrors `tests/workflow/group_widget_endpoints.rs` and the entity-side
//! `tests/skill/access_and_security.rs`. Each test installs one or more
//! SYSTEM-scope skills from an in-test mock hub and creates a group via
//! `POST /groups`, then exercises the group→skills endpoints.
//!
//! B-SK-1 … B-SK-11.

use serde_json::Value as Json;

use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};
use crate::hub::mock_release_server::{MockHub, MockItem, MockVersion, spawn_mock_hub};

/// SKILL.md for a system skill with the given frontmatter `name`. A raw
/// string literal preserves the nested `metadata:` indentation (see the
/// note in `tests/skill/mod.rs`).
fn skill_md(name: &str) -> String {
    format!(
        r#"---
name: {name}
description: Test system skill {name}.
when_to_use: When the test needs a system skill named {name}.
allowed-tools: Read
metadata:
  author: ziee
  license: MIT
---

# {name}

BODY_MARKER for {name}.
"#
    )
}

/// Reverse-DNS hub id for the i-th test skill.
fn hub_id(i: usize) -> String {
    format!("io.github.test/group-skill-{i}")
}

/// A mock catalog carrying `n` distinct skill bundles (each with a unique
/// frontmatter name so each install creates a distinct DB row).
fn multi_skill_catalog(n: usize) -> (Vec<MockVersion>, Vec<String>) {
    // Leak the generated SKILL.md strings so their `&'static str` slices can
    // live in the `MockItem` structs for the duration of the test.
    let mut items = Vec::new();
    for i in 0..n {
        let name = format!("group-skill-{i}");
        let md: &'static str = Box::leak(skill_md(&name).into_boxed_str());
        let hid: &'static str = Box::leak(hub_id(i).into_boxed_str());
        items.push(MockItem::bundle("skill", hid, vec![("SKILL.md", md)]));
    }
    (
        vec![MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items,
        }],
        (0..n).map(hub_id).collect(),
    )
}

/// Boot a TestServer wired to a mock hub serving `n` skill bundles.
async fn server_with_n_skills(n: usize) -> (TestServer, MockHub, Vec<String>) {
    let (versions, ids) = multi_skill_catalog(n);
    let mock = spawn_mock_hub(versions).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    (server, mock, ids)
}

/// Admin with everything the group-assignment path needs (hub refresh +
/// install user & system skills + assign to groups + create/read groups).
async fn admin(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
            "skills::manage_system",
            "skills::assign_to_groups",
            "groups::read",
            "groups::create",
        ],
    )
    .await
}

async fn refresh(server: &TestServer, token: &str) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("refresh");
    assert_eq!(resp.status(), 200, "hub refresh must 200");
}

/// Install one skill on the SYSTEM endpoint; return its id.
async fn install_system_skill(server: &TestServer, token: &str, hub_id: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/system/install-from-hub"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "hub_id": hub_id }))
        .send()
        .await
        .expect("install system skill");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse install body");
    assert_eq!(status, 201, "system skill install should 201; got {status}: {body}");
    assert_eq!(body["skill"]["scope"], Json::String("system".into()));
    body["skill"]["id"].as_str().expect("skill id").to_string()
}

/// Install one skill on the USER endpoint; return its (user-scope) id.
async fn install_user_skill(server: &TestServer, token: &str, hub_id: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/install-from-hub"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "hub_id": hub_id }))
        .send()
        .await
        .expect("install user skill");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse install body");
    assert_eq!(status, 201, "user skill install should 201; got {status}: {body}");
    body["skill"]["id"].as_str().expect("skill id").to_string()
}

/// Create a group; return its id.
async fn create_group(server: &TestServer, token: &str, name: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "name": name, "description": "x", "permissions": [] }))
        .send()
        .await
        .expect("create group");
    assert_eq!(resp.status(), 201, "group create should 201");
    let group: Json = resp.json().await.expect("parse group");
    group["id"].as_str().expect("group id").to_string()
}

async fn get_group_skills(server: &TestServer, token: &str, gid: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{gid}/system-skills")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get group skills")
}

async fn put_group_skills(
    server: &TestServer,
    token: &str,
    gid: &str,
    skill_ids: &[&str],
) -> reqwest::Response {
    reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{gid}/system-skills")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "skill_ids": skill_ids }))
        .send()
        .await
        .expect("put group skills")
}

/// Extract the sorted set of skill ids from a `{skills:[...]}` body.
fn ids_of(body: &Json) -> Vec<String> {
    let mut v: Vec<String> = body["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn system_skill_group_roundtrip() {
    let (server, _mock, ids) = server_with_n_skills(1).await;
    let admin = admin(&server, "sk_grp_rt").await;
    refresh(&server, &admin.token).await;
    let sid = install_system_skill(&server, &admin.token, &ids[0]).await;
    let gid = create_group(&server, &admin.token, "sk-rt-grp").await;

    // PUT [sid] → 200, len 1.
    let put = put_group_skills(&server, &admin.token, &gid, &[&sid]).await;
    assert_eq!(put.status(), 200);
    let body: Json = put.json().await.unwrap();
    assert_eq!(ids_of(&body), vec![sid.clone()]);

    // GET → len 1.
    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert_eq!(got.status(), 200);
    assert_eq!(ids_of(&got.json().await.unwrap()), vec![sid.clone()]);

    // PUT [] → 200, empty.
    let clear = put_group_skills(&server, &admin.token, &gid, &[]).await;
    assert_eq!(clear.status(), 200);
    assert!(ids_of(&clear.json().await.unwrap()).is_empty());

    // GET → empty.
    let after = get_group_skills(&server, &admin.token, &gid).await;
    assert!(ids_of(&after.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_skill_group_multi_assign() {
    let (server, _mock, ids) = server_with_n_skills(3).await;
    let admin = admin(&server, "sk_grp_multi").await;
    refresh(&server, &admin.token).await;
    let mut sids = Vec::new();
    for hid in &ids {
        sids.push(install_system_skill(&server, &admin.token, hid).await);
    }
    let gid = create_group(&server, &admin.token, "sk-multi-grp").await;

    let refs: Vec<&str> = sids.iter().map(|s| s.as_str()).collect();
    let put = put_group_skills(&server, &admin.token, &gid, &refs).await;
    assert_eq!(put.status(), 200);
    let mut expected = sids.clone();
    expected.sort();
    assert_eq!(ids_of(&put.json().await.unwrap()), expected);
}

#[tokio::test]
async fn system_skill_group_diff_update() {
    let (server, _mock, ids) = server_with_n_skills(3).await;
    let admin = admin(&server, "sk_grp_diff").await;
    refresh(&server, &admin.token).await;
    let a = install_system_skill(&server, &admin.token, &ids[0]).await;
    let b = install_system_skill(&server, &admin.token, &ids[1]).await;
    let c = install_system_skill(&server, &admin.token, &ids[2]).await;
    let gid = create_group(&server, &admin.token, "sk-diff-grp").await;

    // group = [A,B]
    put_group_skills(&server, &admin.token, &gid, &[&a, &b]).await;
    // PUT [B,C] → removes A, keeps B, adds C.
    let put = put_group_skills(&server, &admin.token, &gid, &[&b, &c]).await;
    assert_eq!(put.status(), 200);
    let mut expected = vec![b.clone(), c.clone()];
    expected.sort();
    assert_eq!(ids_of(&put.json().await.unwrap()), expected);

    // GET confirms exactly {B,C}.
    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert_eq!(ids_of(&got.json().await.unwrap()), expected);
}

#[tokio::test]
async fn system_skill_group_get_empty() {
    let (server, _mock, _ids) = server_with_n_skills(0).await;
    let admin = admin(&server, "sk_grp_empty").await;
    refresh(&server, &admin.token).await;
    let gid = create_group(&server, &admin.token, "sk-empty-grp").await;

    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert_eq!(got.status(), 200);
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_skill_group_rejects_non_system() {
    let (server, _mock, ids) = server_with_n_skills(1).await;
    let admin = admin(&server, "sk_grp_nonsys").await;
    refresh(&server, &admin.token).await;
    // Install as a USER-scope skill.
    let user_sid = install_user_skill(&server, &admin.token, &ids[0]).await;
    let gid = create_group(&server, &admin.token, "sk-nonsys-grp").await;

    let put = put_group_skills(&server, &admin.token, &gid, &[&user_sid]).await;
    assert_eq!(put.status(), 400, "assigning a user-scope skill must 400");

    // Group stays empty.
    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_skill_group_unknown_id() {
    let (server, _mock, _ids) = server_with_n_skills(0).await;
    let admin = admin(&server, "sk_grp_unknown").await;
    refresh(&server, &admin.token).await;
    let gid = create_group(&server, &admin.token, "sk-unknown-grp").await;

    let random = uuid::Uuid::new_v4().to_string();
    let put = put_group_skills(&server, &admin.token, &gid, &[&random]).await;
    assert_eq!(put.status(), 400, "unknown skill id must 400");

    // No partial write.
    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_skill_group_403_get() {
    let (server, _mock, _ids) = server_with_n_skills(0).await;
    let admin = admin(&server, "sk_grp_403g_admin").await;
    refresh(&server, &admin.token).await;
    let gid = create_group(&server, &admin.token, "sk-403g-grp").await;

    // Caller has groups::read + skills::read but NOT skills::assign_to_groups,
    // so a 403 unambiguously comes from the assign_to_groups gate (not a
    // missing groups perm).
    let weak = create_user_with_permissions(
        &server,
        "sk_403g_weak",
        &["skills::read", "groups::read"],
    )
    .await;
    let got = get_group_skills(&server, &weak.token, &gid).await;
    assert_eq!(got.status(), 403);
}

#[tokio::test]
async fn system_skill_group_403_put() {
    let (server, _mock, ids) = server_with_n_skills(1).await;
    let admin = admin(&server, "sk_grp_403p_admin").await;
    refresh(&server, &admin.token).await;
    let sid = install_system_skill(&server, &admin.token, &ids[0]).await;
    let gid = create_group(&server, &admin.token, "sk-403p-grp").await;

    let weak = create_user_with_permissions(
        &server,
        "sk_403p_weak",
        &["skills::read", "groups::read"],
    )
    .await;
    let put = put_group_skills(&server, &weak.token, &gid, &[&sid]).await;
    assert_eq!(put.status(), 403);

    // No write occurred (admin GET is still empty).
    let got = get_group_skills(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_skill_group_unauth_401() {
    let (server, _mock, _ids) = server_with_n_skills(0).await;
    let admin = admin(&server, "sk_grp_401_admin").await;
    refresh(&server, &admin.token).await;
    let gid = create_group(&server, &admin.token, "sk-401-grp").await;

    let get = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{gid}/system-skills")))
        .send()
        .await
        .expect("get no auth");
    assert_eq!(get.status(), 401);

    let put = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{gid}/system-skills")))
        .json(&serde_json::json!({ "skill_ids": [] }))
        .send()
        .await
        .expect("put no auth");
    assert_eq!(put.status(), 401);
}

#[tokio::test]
async fn system_skill_group_bidirectional_consistency() {
    let (server, _mock, ids) = server_with_n_skills(1).await;
    let admin = admin(&server, "sk_grp_bidi").await;
    refresh(&server, &admin.token).await;
    let sid = install_system_skill(&server, &admin.token, &ids[0]).await;
    let gid = create_group(&server, &admin.token, "sk-bidi-grp").await;

    // Assign via the group endpoint.
    put_group_skills(&server, &admin.token, &gid, &[&sid]).await;

    // The entity-side endpoint agrees: the skill's groups include gid.
    let entity_groups: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/skills/system/{sid}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get skill groups")
        .json()
        .await
        .expect("parse");
    assert!(
        entity_groups
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g == &Json::String(gid.clone())),
        "entity-side skill groups must include gid: {entity_groups}"
    );

    // Removal direction also agrees: after clearing via the group endpoint,
    // the entity-side list no longer contains gid.
    put_group_skills(&server, &admin.token, &gid, &[]).await;
    let after: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/skills/system/{sid}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get skill groups")
        .json()
        .await
        .expect("parse");
    assert!(
        !after.as_array().unwrap().iter().any(|g| g == &Json::String(gid.clone())),
        "entity-side skill groups must DROP gid after removal: {after}"
    );
}

#[tokio::test]
async fn system_skill_group_unknown_group_404() {
    let (server, _mock, _ids) = server_with_n_skills(0).await;
    let admin = admin(&server, "sk_grp_404").await;
    refresh(&server, &admin.token).await;
    let bogus = uuid::Uuid::new_v4().to_string();

    let got = get_group_skills(&server, &admin.token, &bogus).await;
    assert_eq!(got.status(), 404, "GET on a nonexistent group should 404");

    let put = put_group_skills(&server, &admin.token, &bogus, &[]).await;
    assert_eq!(put.status(), 404, "PUT on a nonexistent group should 404");
}

#[tokio::test]
async fn system_skill_group_cascade_on_group_delete() {
    let (server, _mock, ids) = server_with_n_skills(1).await;
    let admin = create_user_with_permissions(
        &server,
        "sk_grp_cascade",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
            "skills::manage_system",
            "skills::assign_to_groups",
            "groups::read",
            "groups::create",
            "groups::delete",
        ],
    )
    .await;
    refresh(&server, &admin.token).await;
    let sid = install_system_skill(&server, &admin.token, &ids[0]).await;
    let gid = create_group(&server, &admin.token, "sk-cascade-grp").await;
    put_group_skills(&server, &admin.token, &gid, &[&sid]).await;

    // Delete the group → the group_skills row cascades away.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/groups/{gid}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete group");
    assert!(del.status().is_success(), "group delete should succeed; got {}", del.status());

    // The skill's group list is now empty (FK cascade).
    let entity_groups: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/skills/system/{sid}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get skill groups")
        .json()
        .await
        .expect("parse");
    assert_eq!(
        entity_groups.as_array().map(|a| a.len()),
        Some(0),
        "group_skills row should be gone after group delete: {entity_groups}"
    );
}
