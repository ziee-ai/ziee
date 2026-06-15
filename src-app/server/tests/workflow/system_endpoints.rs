//! Phase 8 G convergence: the canonical workflow REST surface added to
//! match the skills module —
//!   - `PUT /api/workflows/{id}` edits a user-owned workflow,
//!   - `POST /api/workflows/system/import` (admin multipart) installs a
//!     system-scope workflow,
//!   - `GET/POST/DELETE /api/workflows/system/{id}/groups` manage group
//!     assignment, and reject a non-system workflow with 400.

use reqwest::multipart::{Form, Part};
use serde_json::Value as Json;
use uuid::Uuid;

use super::{
    FIXTURE_WORKFLOW_YAML, import_dev_workflow, plain_server, workflow_tarball, workflow_user,
};
use crate::common::test_helpers::create_user_with_permissions;

/// A minimal valid 1-step llm workflow (avoids the sandbox flavor reqs).
const SIMPLE_YAML: &str = r#"inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "say something about {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ gen.output }}"
"#;

#[tokio::test]
async fn put_updates_user_workflow_enabled_toggle() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_put").await;
    let wf = import_dev_workflow(&server, &user.token, "put-target", SIMPLE_YAML).await;
    let id = wf["id"].as_str().unwrap();
    assert_eq!(wf["enabled"], Json::Bool(true));

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/workflows/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "enabled": false, "description": "edited" }))
        .send()
        .await
        .expect("put");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse");
    assert_eq!(status, 200, "PUT should 200; got {status}: {body}");
    assert_eq!(body["enabled"], Json::Bool(false));
    assert_eq!(body["description"], Json::String("edited".into()));
}

#[tokio::test]
async fn put_rejects_non_owner() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_owner").await;
    let other = workflow_user(&server, "wf_other").await;
    let wf = import_dev_workflow(&server, &owner.token, "owned", SIMPLE_YAML).await;
    let id = wf["id"].as_str().unwrap();

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/workflows/{id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&serde_json::json!({ "enabled": false }))
        .send()
        .await
        .expect("put");
    assert_eq!(resp.status(), 403, "non-owner PUT should 403");
}

#[tokio::test]
async fn system_import_installs_system_scope() {
    let server = plain_server().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_sys_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::execute",
        ],
    )
    .await;

    let tarball = workflow_tarball(SIMPLE_YAML);
    let part = Part::bytes(tarball)
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = Form::new().part("bundle", part);
    let resp = reqwest::Client::new()
        .post(server.api_url("/workflows/system/import?name=sysflow"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .multipart(form)
        .send()
        .await
        .expect("system import");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse");
    assert_eq!(status, 201, "system import should 201; got {status}: {body}");
    assert_eq!(body["scope"], Json::String("system".into()));
    assert!(body["owner_user_id"].is_null(), "system scope has no owner");
}

#[tokio::test]
async fn system_import_requires_admin() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_nonadmin").await; // no manage_system

    let tarball = workflow_tarball(SIMPLE_YAML);
    let part = Part::bytes(tarball)
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = Form::new().part("bundle", part);
    let resp = reqwest::Client::new()
        .post(server.api_url("/workflows/system/import?name=denied"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("system import");
    assert_eq!(
        resp.status(),
        403,
        "system import without manage_system should 403"
    );
}

#[tokio::test]
async fn group_assignment_rejects_non_system_workflow() {
    let server = plain_server().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_groups_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::assign_to_groups",
            "workflows::execute",
        ],
    )
    .await;

    // A USER-scope workflow (dev import default scope).
    let wf = import_dev_workflow(&server, &admin.token, "user-scope-wf", SIMPLE_YAML).await;
    let id = wf["id"].as_str().unwrap();
    assert_eq!(wf["scope"], Json::String("user".into()));

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/system/{id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "group_ids": [Uuid::new_v4()] }))
        .send()
        .await
        .expect("set groups");
    assert_eq!(
        resp.status(),
        400,
        "assigning groups to a non-system workflow should 400"
    );
}

#[tokio::test]
async fn system_workflow_group_roundtrip() {
    let server = plain_server().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_group_rt_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::assign_to_groups",
            "groups::read",
            "groups::create",
        ],
    )
    .await;

    // Install a SYSTEM workflow via the admin import.
    let tarball = workflow_tarball(SIMPLE_YAML);
    let part = Part::bytes(tarball)
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = Form::new().part("bundle", part);
    let body: Json = reqwest::Client::new()
        .post(server.api_url("/workflows/system/import?name=grouped"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .multipart(form)
        .send()
        .await
        .expect("system import")
        .json()
        .await
        .expect("parse");
    let wf_id = body["id"].as_str().unwrap();

    // Create a group to assign.
    let grp_resp = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({
            "name": "wf-grp",
            "description": "x",
            "permissions": [],
        }))
        .send()
        .await
        .expect("create group");
    assert_eq!(grp_resp.status(), 201, "group create should 201");
    let group: Json = grp_resp.json().await.expect("parse group");
    let gid = group["id"].as_str().expect("group id");

    // POST set groups → 204.
    let set = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/system/{wf_id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "group_ids": [gid] }))
        .send()
        .await
        .expect("set groups");
    assert_eq!(set.status(), 204, "set groups should 204");

    // GET groups → [gid].
    let got: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/system/{wf_id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get groups")
        .json()
        .await
        .expect("parse groups");
    assert_eq!(got.as_array().map(|a| a.len()), Some(1));
    assert_eq!(got[0], Json::String(gid.into()));

    // DELETE one group → 204, then GET is empty.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflows/system/{wf_id}/groups/{gid}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete group");
    assert_eq!(del.status(), 204, "delete group should 204");

    let after: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/system/{wf_id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get groups")
        .json()
        .await
        .expect("parse groups");
    assert_eq!(after.as_array().map(|a| a.len()), Some(0));

    let _ = FIXTURE_WORKFLOW_YAML; // keep the import available for other tests
}
