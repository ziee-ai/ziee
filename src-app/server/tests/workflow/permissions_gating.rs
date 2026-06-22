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
