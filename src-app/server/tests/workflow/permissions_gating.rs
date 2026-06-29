//! Audit gap S8: negative permission-gating on the workflow REST surface.
//!
//! Mirrors `system_endpoints.rs::system_import_requires_admin` — assert the
//! 403 a caller WITHOUT the gating permission gets, distinct from the
//! ownership/scope 403s the other tests cover.
//!
//!   * `POST /api/workflows/import` (dev import) is gated on `workflows::install`
//!     (`dev::import_workflow` → `RequirePermissions<(WorkflowsInstall,)>`); a
//!     user lacking it → 403.
//!   * `POST /api/workflows/system/{id}/groups` (assign-to-groups) is gated on
//!     `workflows::assign_to_groups` (`system::set_workflow_groups` →
//!     `RequirePermissions<(WorkflowsAssignToGroups,)>`); a user lacking it → 403,
//!     and the perm guard fires BEFORE the body/scope validation (so any uuid
//!     path + body is fine).

use reqwest::multipart::{Form, Part};
use uuid::Uuid;

use super::{SIMPLE_OK_YAML, plain_server, workflow_tarball};
use crate::common::test_helpers::create_user_with_permissions;

#[tokio::test]
async fn dev_import_requires_install_permission() {
    let server = plain_server().await;
    // A user with read/execute but explicitly NOT `workflows::install` —
    // so a 403 is the install-permission gate, not an auth failure.
    let user = create_user_with_permissions(
        &server,
        "wf_import_noinstall",
        &["workflows::read", "workflows::execute"],
    )
    .await;

    let tarball = workflow_tarball(SIMPLE_OK_YAML);
    let part = Part::bytes(tarball)
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = Form::new().part("bundle", part);
    let resp = reqwest::Client::new()
        .post(server.api_url("/workflows/import?name=denied-import"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("dev import without install perm");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 403,
        "dev import without workflows::install should 403: {body}"
    );
}

#[tokio::test]
async fn assign_to_groups_requires_assign_permission() {
    let server = plain_server().await;
    // An admin-ish user with manage_system (so they CAN install system
    // workflows) but NOT `workflows::assign_to_groups` — the perm gate on
    // the groups endpoint fires before any scope/body check.
    let user = create_user_with_permissions(
        &server,
        "wf_assign_noperm",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::execute",
        ],
    )
    .await;

    // The `WorkflowsAssignToGroups` extractor 403s before the handler body,
    // so an arbitrary workflow id + group id never gets looked up.
    let wf_id = Uuid::new_v4();
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/system/{wf_id}/groups")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "group_ids": [Uuid::new_v4()] }))
        .send()
        .await
        .expect("set groups without assign perm");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 403,
        "assign-to-groups without workflows::assign_to_groups should 403: {body}"
    );
}

// audit id all-09d02d703669 — run-level endpoints (run a workflow, change a
// run's timeout, delete a run) are all gated on `workflows::execute`, but only
// import/groups had negative perm-gate tests. A user WITHOUT workflows::execute
// must 403 on each; the perm extractor fires before any workflow/run lookup, so
// arbitrary ids are fine.
#[tokio::test]
async fn run_level_endpoints_require_execute_permission() {
    let server = plain_server().await;
    // read-only (+ install/manage) but explicitly NOT workflows::execute.
    // `_only_` so the user does NOT inherit the default Users group (which
    // grants workflows::execute) — otherwise the perm gate can't fire.
    let user = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "wf_run_noexec",
        &["workflows::read", "workflows::install", "workflows::manage"],
    )
    .await;
    let auth = format!("Bearer {}", user.token);
    let client = reqwest::Client::new();

    // POST /workflows/{id}/run
    let wf_id = Uuid::new_v4();
    let resp = client
        .post(server.api_url(&format!("/workflows/{wf_id}/run")))
        .header("Authorization", &auth)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("run without execute perm");
    assert_eq!(resp.status(), 403, "run_workflow must require workflows::execute");

    // PUT /workflow-runs/{run_id}/timeout
    let run_id = Uuid::new_v4();
    let resp = client
        .put(server.api_url(&format!("/workflow-runs/{run_id}/timeout")))
        .header("Authorization", &auth)
        .json(&serde_json::json!({ "timeout_ms": 1000 }))
        .send()
        .await
        .expect("set timeout without execute perm");
    assert_eq!(resp.status(), 403, "set_run_timeout must require workflows::execute");

    // DELETE /workflow-runs/{run_id}
    let resp = client
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", &auth)
        .send()
        .await
        .expect("delete run without execute perm");
    assert_eq!(resp.status(), 403, "delete_run must require workflows::execute");
}
