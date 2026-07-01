//! Group-centric system-workflow assignment endpoints (the User Groups page
//! widget direction): `GET/PUT /api/groups/{group_id}/system-workflows`.
//!
//! Mirrors `tests/skill/group_endpoints.rs` and the entity-side
//! `tests/workflow/system_endpoints.rs::system_workflow_group_roundtrip`.
//! Each test installs SYSTEM-scope workflows via the admin import helper and
//! creates a group via `POST /groups`.
//!
//! B-WF-1 … B-WF-11. B-WF-5 is CRITICAL: it proves the handler's scope guard
//! returns 400 BEFORE the `group_workflows` DB trigger would 500.

use serde_json::Value as Json;

use super::{SIMPLE_OK_YAML, import_dev_workflow, plain_server, system_import_workflow};
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

/// Admin with everything the group-assignment path needs.
async fn admin(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::assign_to_groups",
            "workflows::execute",
            "groups::read",
            "groups::create",
        ],
    )
    .await
}

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

async fn get_group_workflows(server: &TestServer, token: &str, gid: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{gid}/system-workflows")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get group workflows")
}

async fn put_group_workflows(
    server: &TestServer,
    token: &str,
    gid: &str,
    workflow_ids: &[&str],
) -> reqwest::Response {
    reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{gid}/system-workflows")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "workflow_ids": workflow_ids }))
        .send()
        .await
        .expect("put group workflows")
}

/// Extract the sorted set of workflow ids from a `{workflows:[...]}` body.
fn ids_of(body: &Json) -> Vec<String> {
    let mut v: Vec<String> = body["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .map(|w| w["id"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

/// Install a SYSTEM workflow with a unique slug; return its id.
async fn install_system_wf(server: &TestServer, token: &str, slug: &str) -> String {
    let body = system_import_workflow(server, token, slug, SIMPLE_OK_YAML).await;
    assert_eq!(body["scope"], Json::String("system".into()));
    body["id"].as_str().expect("workflow id").to_string()
}

#[tokio::test]
async fn system_workflow_group_roundtrip() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_rt").await;
    let wid = install_system_wf(&server, &admin.token, "wf-rt").await;
    let gid = create_group(&server, &admin.token, "wf-rt-grp").await;

    let put = put_group_workflows(&server, &admin.token, &gid, &[&wid]).await;
    assert_eq!(put.status(), 200);
    assert_eq!(ids_of(&put.json().await.unwrap()), vec![wid.clone()]);

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert_eq!(got.status(), 200);
    assert_eq!(ids_of(&got.json().await.unwrap()), vec![wid.clone()]);

    let clear = put_group_workflows(&server, &admin.token, &gid, &[]).await;
    assert_eq!(clear.status(), 200);
    assert!(ids_of(&clear.json().await.unwrap()).is_empty());

    let after = get_group_workflows(&server, &admin.token, &gid).await;
    assert!(ids_of(&after.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_workflow_group_multi_assign() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_multi").await;
    let a = install_system_wf(&server, &admin.token, "wf-m-a").await;
    let b = install_system_wf(&server, &admin.token, "wf-m-b").await;
    let c = install_system_wf(&server, &admin.token, "wf-m-c").await;
    let gid = create_group(&server, &admin.token, "wf-multi-grp").await;

    let put = put_group_workflows(&server, &admin.token, &gid, &[&a, &b, &c]).await;
    assert_eq!(put.status(), 200);
    let mut expected = vec![a, b, c];
    expected.sort();
    assert_eq!(ids_of(&put.json().await.unwrap()), expected);
}

#[tokio::test]
async fn system_workflow_group_diff_update() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_diff").await;
    let a = install_system_wf(&server, &admin.token, "wf-d-a").await;
    let b = install_system_wf(&server, &admin.token, "wf-d-b").await;
    let c = install_system_wf(&server, &admin.token, "wf-d-c").await;
    let gid = create_group(&server, &admin.token, "wf-diff-grp").await;

    put_group_workflows(&server, &admin.token, &gid, &[&a, &b]).await;
    let put = put_group_workflows(&server, &admin.token, &gid, &[&b, &c]).await;
    assert_eq!(put.status(), 200);
    let mut expected = vec![b.clone(), c.clone()];
    expected.sort();
    assert_eq!(ids_of(&put.json().await.unwrap()), expected);

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert_eq!(ids_of(&got.json().await.unwrap()), expected);
}

#[tokio::test]
async fn system_workflow_group_get_empty() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_empty").await;
    let gid = create_group(&server, &admin.token, "wf-empty-grp").await;

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert_eq!(got.status(), 200);
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_workflow_group_rejects_non_system() {
    // CRITICAL: the handler scope guard must fire (400) BEFORE the
    // group_workflows DB trigger would surface as a 500.
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_nonsys").await;
    // A USER-scope workflow (dev import default scope).
    let wf = import_dev_workflow(&server, &admin.token, "wf-nonsys", SIMPLE_OK_YAML).await;
    assert_eq!(wf["scope"], Json::String("user".into()));
    let user_wid = wf["id"].as_str().unwrap();
    let gid = create_group(&server, &admin.token, "wf-nonsys-grp").await;

    let put = put_group_workflows(&server, &admin.token, &gid, &[user_wid]).await;
    assert_eq!(put.status(), 400, "assigning a user-scope workflow must 400, not 500");

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_workflow_group_unknown_id() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_unknown").await;
    let gid = create_group(&server, &admin.token, "wf-unknown-grp").await;

    let random = uuid::Uuid::new_v4().to_string();
    let put = put_group_workflows(&server, &admin.token, &gid, &[&random]).await;
    assert_eq!(put.status(), 400, "unknown workflow id must 400");

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_workflow_group_403_get() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_403g_admin").await;
    let gid = create_group(&server, &admin.token, "wf-403g-grp").await;

    let weak = create_user_with_permissions(&server, "wf_403g_weak", &["workflows::read"]).await;
    let got = get_group_workflows(&server, &weak.token, &gid).await;
    assert_eq!(got.status(), 403);
}

#[tokio::test]
async fn system_workflow_group_403_put() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_403p_admin").await;
    let wid = install_system_wf(&server, &admin.token, "wf-403p").await;
    let gid = create_group(&server, &admin.token, "wf-403p-grp").await;

    let weak = create_user_with_permissions(&server, "wf_403p_weak", &["workflows::read"]).await;
    let put = put_group_workflows(&server, &weak.token, &gid, &[&wid]).await;
    assert_eq!(put.status(), 403);

    let got = get_group_workflows(&server, &admin.token, &gid).await;
    assert!(ids_of(&got.json().await.unwrap()).is_empty());
}

#[tokio::test]
async fn system_workflow_group_unauth_401() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_401_admin").await;
    let gid = create_group(&server, &admin.token, "wf-401-grp").await;

    let get = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{gid}/system-workflows")))
        .send()
        .await
        .expect("get no auth");
    assert_eq!(get.status(), 401);

    let put = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{gid}/system-workflows")))
        .json(&serde_json::json!({ "workflow_ids": [] }))
        .send()
        .await
        .expect("put no auth");
    assert_eq!(put.status(), 401);
}

#[tokio::test]
async fn system_workflow_group_bidirectional_consistency() {
    let server = plain_server().await;
    let admin = admin(&server, "wf_grp_bidi").await;
    let wid = install_system_wf(&server, &admin.token, "wf-bidi").await;
    let gid = create_group(&server, &admin.token, "wf-bidi-grp").await;

    put_group_workflows(&server, &admin.token, &gid, &[&wid]).await;

    let entity_groups: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/system/{wid}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get workflow groups")
        .json()
        .await
        .expect("parse");
    assert!(
        entity_groups
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g == &Json::String(gid.clone())),
        "entity-side workflow groups must include gid: {entity_groups}"
    );
}

#[tokio::test]
async fn system_workflow_group_cascade_on_group_delete() {
    let server = plain_server().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_grp_cascade",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::assign_to_groups",
            "workflows::execute",
            "groups::read",
            "groups::create",
            "groups::delete",
        ],
    )
    .await;
    let wid = install_system_wf(&server, &admin.token, "wf-cascade").await;
    let gid = create_group(&server, &admin.token, "wf-cascade-grp").await;
    put_group_workflows(&server, &admin.token, &gid, &[&wid]).await;

    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/groups/{gid}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete group");
    assert!(del.status().is_success(), "group delete should succeed; got {}", del.status());

    let entity_groups: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/system/{wid}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get workflow groups")
        .json()
        .await
        .expect("parse");
    assert_eq!(
        entity_groups.as_array().map(|a| a.len()),
        Some(0),
        "group_workflows row should be gone after group delete: {entity_groups}"
    );
}
