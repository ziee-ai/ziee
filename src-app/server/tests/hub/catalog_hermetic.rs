//! Hermetic hub catalog tests — no network, no real cosign.
//!
//! Uses the in-test `mock_release_server` + the debug-only fetch
//! overrides so the full activate → fetch → sha256 → unpack → rotate
//! path is exercised against a local server. Replaces the
//! network-dependent assertions the plan flagged.

use serde_json::{json, Value as Json};

use super::mock_release_server::{spawn_mock_hub, MockItem, MockVersion};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn two_versions() -> Vec<MockVersion> {
    vec![
        // Newest-first (GitHub order). v9.9.2 adds an incompatible
        // assistant (min_ziee_version 99.0.0).
        MockVersion {
            version: "9.9.2-test",
            prerelease: true,
            items: vec![
                MockItem { category: "model", id: "mock-model-a", min_ziee_version: None },
                MockItem { category: "assistant", id: "mock-asst-a", min_ziee_version: None },
                MockItem {
                    category: "assistant",
                    id: "mock-asst-future",
                    min_ziee_version: Some("99.0.0"),
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![
                MockItem { category: "model", id: "mock-model-a", min_ziee_version: None },
                MockItem { category: "assistant", id: "mock-asst-a", min_ziee_version: None },
            ],
        },
    ]
}

#[tokio::test]
async fn activate_then_switch_versions_against_mock() {
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::models::read"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate the older mock version (2 items).
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate 9.9.1");
    assert_eq!(
        resp.status(),
        200,
        "activate 9.9.1-test (unsigned mock) should succeed: {}",
        resp.text().await.unwrap_or_default()
    );

    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("index")
        .json()
        .await
        .expect("parse index");
    assert_eq!(idx["hub_version"], "9.9.1-test");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(2));

    // /version reports source=github (a fetch replaced the seed) +
    // cosign was skipped (unsigned mock).
    let ver: Json = client
        .get(server.api_url("/hub/version"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("version")
        .json()
        .await
        .expect("parse version");
    assert_eq!(ver["source"], "github");
    assert_eq!(ver["hub_version"], "9.9.1-test");

    // Switch to the newer version (3 items).
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate 9.9.2");
    assert_eq!(resp.status(), 200);
    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("index 2")
        .json()
        .await
        .expect("parse index 2");
    assert_eq!(idx["hub_version"], "9.9.2-test");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(3));
}

#[tokio::test]
async fn install_rejects_incompatible_item() {
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::assistants::create"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate v9.9.2 which contains the future-pinned assistant.
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate");
    assert_eq!(resp.status(), 200);

    // A compatible assistant installs fine (201).
    let ok = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("create compatible");
    assert_eq!(
        ok.status(),
        201,
        "compatible assistant should install: {}",
        ok.text().await.unwrap_or_default()
    );

    // The future-pinned assistant is rejected by the compat gate (422),
    // even though the API client bypasses the UI hiding.
    let blocked = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-future" }))
        .send()
        .await
        .expect("create incompatible");
    assert_eq!(
        blocked.status(),
        422,
        "incompatible assistant should be 422'd, got {}",
        blocked.status()
    );
    let body: Json = blocked.json().await.expect("parse 422 body");
    assert_eq!(body["error_code"], "HUB_INCOMPATIBLE");
}

#[tokio::test]
async fn install_stamps_current_version_so_updates_stays_empty() {
    // Directly exercises the hub_version write-back: installing from the
    // active catalog must stamp hub_entities.hub_version with the current
    // version, so /hub/updates does NOT immediately flag the fresh install
    // as "behind". (Regression guard for the bug where every install was
    // recorded with NULL and showed as needing an update.)
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::assistants::create"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate v9.9.1-test, then install a compatible assistant from it.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");
    client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");

    // The fresh install was stamped 9.9.1-test == current → not behind.
    let updates: Json = client
        .get(server.api_url("/hub/updates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("updates")
        .json()
        .await
        .expect("parse updates");
    let rows = updates["updates"].as_array().expect("updates array");
    assert!(
        rows.iter().all(|r| r["hub_id"] != "mock-asst-a"),
        "freshly-installed item must NOT appear in updates: {updates}"
    );
}

// ============================================================================
// /hub/assistant-templates/create — system-wide template install
// ============================================================================
//
// Companion endpoint to /hub/assistants/create. Same hub-load + verify +
// track flow, but creates a TEMPLATE (`is_template=true, created_by=NULL`,
// enforced by the `template_must_have_no_owner` CHECK constraint in
// migration 6) gated on `hub::assistants::create + assistant_templates::create`.

#[tokio::test]
async fn install_as_template_creates_template_with_null_owner() {
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "hub::assistants::create",
            "assistant_templates::create",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate v9.9.1 — both compatible assistants are installable here.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");

    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("create template");
    assert_eq!(
        resp.status(),
        201,
        "template install should succeed: {}",
        resp.text().await.unwrap_or_default()
    );
    let body: Json = resp.json().await.expect("parse template body");
    // The created assistant must be a template with no owner.
    assert_eq!(
        body["assistant"]["is_template"], true,
        "is_template must be true: {body}",
    );
    assert!(
        body["assistant"]["created_by"].is_null(),
        "created_by must be null for templates: {body}",
    );
    // The hub_entities row also records created_by=NULL for the
    // system-wide install (matches the per-user `created_by:
    // Some(user)` path used by /hub/assistants/create).
    assert!(
        body["hub_tracking"]["created_by"].is_null(),
        "hub_tracking.created_by must be null for template installs: {body}",
    );
}

#[tokio::test]
async fn install_as_template_requires_template_permission() {
    // User has hub::assistants::create but NOT
    // assistant_templates::create — endpoint requires BOTH, so 403.
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let user_no_template = create_user_with_permissions(
        &server,
        "user_no_template",
        &["hub::catalog::read", "hub::assistants::create"],
    )
    .await;
    let client = reqwest::Client::new();

    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header(
            "Authorization",
            format!("Bearer {}", user_no_template.token),
        )
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("create template no perm");

    assert_eq!(
        resp.status(),
        403,
        "missing assistant_templates::create → 403"
    );
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

// NOTE: there's no companion test for "user missing
// `hub::assistants::create` → 403" because migration 27
// (`fix_default_user_permissions.sql`) grants `hub::assistants::create`
// to every user by default. The scenario is unreachable without
// custom permission seeding the test harness doesn't expose.
// `assistant_templates::create` is the discriminating permission
// (admin-only) — its absence is covered by the test above.

#[tokio::test]
async fn install_as_template_rejects_incompatible_item() {
    // Defense-in-depth: the same compat gate that the user-install
    // path applies (ensure_installable) also fires for templates.
    // The future-pinned mock assistant exists in v9.9.2 with
    // min_ziee_version above the server, so install must 422.
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "hub::assistants::create",
            "assistant_templates::create",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");

    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-future" }))
        .send()
        .await
        .expect("create incompatible template");

    assert_eq!(resp.status(), 422);
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["error_code"], "HUB_INCOMPATIBLE");
}
