//! Install the seed-shaped fixture workflow from the (mock) hub catalog →
//! assert the full bundle pipeline: DB row + on-disk extract +
//! workflow.yaml present + Layer 1+2+3 validate (cycle-check) passed +
//! hub_entities tracking row.
//!
//! The fixture mirrors the embedded seed `research-summarize-write`
//! 3-step sequential workflow. Because the install handler parses +
//! cycle-checks `workflow.yaml` before inserting the row, a successful
//! 201 install IS the proof the validation pipeline accepted it.

use serde_json::Value as Json;

use super::{
    FIXTURE_WORKFLOW_NAME, admin_and_refresh, install_fixture_workflow,
    server_with_workflow_catalog,
};
use crate::common::test_helpers::create_user_with_permissions;

#[tokio::test]
async fn user_install_creates_row_extract_and_tracking() {
    let (server, _mock) = server_with_workflow_catalog().await;
    let admin = admin_and_refresh(&server).await;

    let body = install_fixture_workflow(&server, &admin.token).await;
    let wf = &body["workflow"];

    // Row identity + scope.
    assert_eq!(wf["name"], FIXTURE_WORKFLOW_NAME, "name persisted: {body}");
    assert_eq!(wf["scope"], "user", "user endpoint forces scope=user: {body}");
    assert!(
        wf["owner_user_id"].is_string(),
        "user-scope workflow must have an owner: {body}"
    );
    assert_eq!(wf["entry_point"], "workflow.yaml", "entry_point: {body}");
    assert_eq!(wf["is_dev"], false, "hub install is not is_dev: {body}");

    // Phase 8 G pattern (d): the validator's compile pass populates
    // compiled_ir_json at install time (it was always NULL before). Assert
    // the column is no longer null and carries a well-formed IR.
    let ir = &wf["compiled_ir_json"];
    assert!(
        !ir.is_null(),
        "compiled_ir_json must be populated on install: {body}"
    );
    assert_eq!(ir["ir_version"], 1, "IR version stamped: {body}");
    assert_eq!(
        ir["step_count"], 3,
        "IR captures the 3 steps of the fixture workflow: {body}"
    );
    assert!(
        ir["steps"].is_array() && ir["topo_order"].is_array(),
        "IR carries steps[] + topo_order[]: {body}"
    );

    // On-disk extract: workflow.yaml present at extracted_path. The
    // install handler already parsed + cycle-checked it (the 201 proves
    // validation passed), so reading the file back confirms the bundle
    // landed on disk.
    let extracted_path = wf["extracted_path"].as_str().expect("extracted_path string");
    let wf_yaml = std::path::Path::new(extracted_path).join("workflow.yaml");
    assert!(
        wf_yaml.exists(),
        "workflow.yaml must exist on disk at {}",
        wf_yaml.display()
    );
    let on_disk = std::fs::read_to_string(&wf_yaml).expect("read workflow.yaml");
    assert!(
        on_disk.contains("research") && on_disk.contains("summarize") && on_disk.contains("write"),
        "extracted workflow.yaml carries the 3 steps"
    );

    // file_count + bundle_sha256 recorded (single-file bundle).
    assert_eq!(wf["file_count"], 1, "one file in the bundle: {body}");
    assert!(
        wf["bundle_sha256"].as_str().unwrap_or("").len() == 64,
        "bundle_sha256 is a 64-char hex digest: {body}"
    );

    // Hub tracking row.
    let tracking = &body["hub_tracking"];
    assert_eq!(tracking["entity_type"], "workflow", "tracking entity_type: {body}");
    assert_eq!(tracking["hub_category"], "workflow", "tracking hub_category: {body}");
    assert_eq!(tracking["hub_id"], FIXTURE_WORKFLOW_NAME, "tracking hub_id: {body}");

    // The workflow now appears in GET /workflows.
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list workflows")
        .json()
        .await
        .expect("parse list");
    let found = list["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .any(|w| w["name"] == FIXTURE_WORKFLOW_NAME);
    assert!(found, "installed workflow appears in GET /workflows: {list}");
}

#[tokio::test]
async fn delete_removes_extracted_dir() {
    let (server, _mock) = server_with_workflow_catalog().await;
    let admin = admin_and_refresh(&server).await;

    let body = install_fixture_workflow(&server, &admin.token).await;
    let wf = &body["workflow"];
    let id = wf["id"].as_str().expect("id").to_string();
    let extracted_path = wf["extracted_path"].as_str().expect("extracted_path").to_string();
    assert!(
        std::path::Path::new(&extracted_path).exists(),
        "extracted dir present before delete"
    );

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflows/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete workflow");
    assert_eq!(resp.status(), 204, "delete should 204");

    // DELETE must rm -rf the extracted_path (plan §3 + §8.9).
    assert!(
        !std::path::Path::new(&extracted_path).exists(),
        "DELETE must remove the extracted dir at {extracted_path}"
    );
}

/// SYSTEM install-from-hub (`POST /hub/workflows/create-system`, perm
/// `workflows::manage_system`) — distinct from the multipart
/// `/workflows/system/import` path that `system_endpoints.rs` covers. The
/// hub path runs `build_workflow_create_from_hub("system", ..)` →
/// `install_system_workflow_tx`, forcing scope=system + null owner and
/// emitting the system-workflow sync event. A 201 proves the same
/// download → sha256 → extract → workflow.yaml parse → Layer 1+2+3
/// validate pipeline ran, then persisted at system scope.
#[tokio::test]
async fn system_install_from_hub_creates_system_scope() {
    let (server, _mock) = server_with_workflow_catalog().await;
    let admin = admin_and_refresh(&server).await; // has workflows::manage_system

    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/workflows/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "hub_id": FIXTURE_WORKFLOW_NAME, "groups": [] }))
        .send()
        .await
        .expect("system install-from-hub");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse system install body");
    assert_eq!(
        status, 201,
        "system install-from-hub should 201; got {status}: {body}"
    );

    let wf = &body["workflow"];
    assert_eq!(wf["name"], FIXTURE_WORKFLOW_NAME, "name persisted: {body}");
    assert_eq!(
        wf["scope"], "system",
        "create-system forces scope=system: {body}"
    );
    assert!(
        wf["owner_user_id"].is_null(),
        "system-scope workflow has no owner: {body}"
    );
    assert_eq!(wf["is_dev"], false, "hub install is not is_dev: {body}");
    // The same parse + compile pipeline as the user path runs at install.
    assert_eq!(
        wf["compiled_ir_json"]["step_count"], 3,
        "IR captures the 3 steps of the fixture workflow: {body}"
    );

    // Hub tracking row stamped for the workflow entity.
    let tracking = &body["hub_tracking"];
    assert_eq!(tracking["entity_type"], "workflow", "tracking entity_type: {body}");
    assert_eq!(tracking["hub_id"], FIXTURE_WORKFLOW_NAME, "tracking hub_id: {body}");

    // The system workflow now appears in GET /workflows for the admin.
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list workflows")
        .json()
        .await
        .expect("parse list");
    let found = list["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .any(|w| w["name"] == FIXTURE_WORKFLOW_NAME && w["scope"] == "system");
    assert!(
        found,
        "installed system workflow appears in GET /workflows: {list}"
    );
}

/// The system hub-install endpoint is gated on `workflows::manage_system`.
/// A user who can install a USER workflow (`workflows::install`) but lacks
/// `manage_system` must be refused with 403 — and no system row is created.
#[tokio::test]
async fn system_install_from_hub_requires_manage_system() {
    let (server, _mock) = server_with_workflow_catalog().await;
    // Refresh the catalog as a full admin first so the manifest is active.
    let _admin = admin_and_refresh(&server).await;

    // A non-admin who can install user workflows but NOT manage_system.
    let user = create_user_with_permissions(
        &server,
        "wf_hub_nonadmin",
        &[
            "hub::catalog::read",
            "workflows::read",
            "workflows::install",
            "workflows::execute",
        ],
    )
    .await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/workflows/create-system"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "hub_id": FIXTURE_WORKFLOW_NAME, "groups": [] }))
        .send()
        .await
        .expect("system install-from-hub (non-admin)");
    assert_eq!(
        resp.status(),
        403,
        "system install without manage_system must 403"
    );
}
