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

#[tokio::test]
async fn install_as_template_duplicate_is_409() {
    // Idempotency guard: a second install with the same hub_id and
    // no `replace_existing` flag must 409 — otherwise the admin
    // accidentally creates duplicate templates that both fan out to
    // every new user via the clone-on-signup hook.
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
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");

    // First install succeeds.
    let first = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("first install");
    assert_eq!(first.status(), 201);

    // Second install (no replace_existing) → 409.
    let second = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("second install");
    assert_eq!(
        second.status(),
        409,
        "duplicate template install should 409: {}",
        second.text().await.unwrap_or_default(),
    );
    let body: Json = second.json().await.unwrap();
    assert_eq!(body["error_code"], "RESOURCE_CONFLICT");
}

#[tokio::test]
async fn install_as_template_with_replace_existing_succeeds() {
    // `replace_existing: true` deletes the prior template + creates
    // afresh. The new assistant has a different uuid, and the prior
    // assistants row is gone (cleanup is event-driven via
    // CleanupHubEntitiesHandler so the dangling hub_entities row is
    // also removed before the new track_hub_entity runs).
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
            "hub::assistants::read",
            "hub::assistants::create",
            "assistant_templates::create",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");

    let first: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    let first_id = first["assistant"]["id"].as_str().unwrap().to_string();

    let second: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a", "replace_existing": true }))
        .send()
        .await
        .expect("replace install")
        .json()
        .await
        .expect("parse second");
    let second_id = second["assistant"]["id"].as_str().unwrap().to_string();
    assert_ne!(first_id, second_id, "replace must produce a new uuid");

    // The prior template is gone — `created_template_ids` on the hub
    // listing reflects only the NEW id (INNER JOIN against assistants
    // in get_template_install_ids self-cleans deletions, and the
    // CleanupHubEntitiesHandler removes the dangling hub_entities row
    // event-driven from the AssistantEvent::Deleted emit).
    let listing: Json = client
        .get(server.api_url("/hub/assistants?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after replace")
        .json()
        .await
        .expect("parse listing");
    let row = listing
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "mock-asst-a")
        .expect("mock-asst-a in listing");
    let ids: Vec<String> = row["created_template_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![second_id.clone()], "only NEW id should remain");
    assert!(
        !ids.contains(&first_id),
        "old id must be gone from created_template_ids",
    );

    // A THIRD `replace_existing` install should also succeed — the
    // find_template_install JOIN guards against orphan rows.
    let third = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a", "replace_existing": true }))
        .send()
        .await
        .expect("third install");
    assert_eq!(third.status(), 201, "third replace must still 201");
}

#[tokio::test]
async fn template_install_surfaces_in_updates_with_template_flag() {
    // When the catalog version moves forward AFTER a template install,
    // /hub/updates surfaces the row with `is_template_install: true`
    // so the UI routes the Re-install action through the template
    // endpoint (not the user-install endpoint, which would silently
    // demote the template to a user-owned assistant).
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

    // Activate v9.9.1, install template, then activate v9.9.2 so the
    // template's hub_version (9.9.1) is now behind.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate v1")
        .error_for_status()
        .expect("activate v1 ok");
    client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate v2")
        .error_for_status()
        .expect("activate v2 ok");

    let updates: Json = client
        .get(server.api_url("/hub/updates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("updates")
        .json()
        .await
        .expect("parse updates");
    let row = updates["updates"]
        .as_array()
        .expect("updates array")
        .iter()
        .find(|r| r["hub_id"] == "mock-asst-a")
        .expect("template install must appear in updates");
    assert_eq!(
        row["is_template_install"], true,
        "template install must be flagged: {row}",
    );
}

#[tokio::test]
async fn template_install_appears_in_created_template_ids_on_get_assistants() {
    // After installing a hub assistant as a template, GET /hub/assistants
    // must populate `created_template_ids` so the UI can disable the
    // "Use as Template" button + show "Template Installed" without a
    // separate round-trip.
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
            "hub::assistants::read",
            "hub::assistants::create",
            "assistant_templates::create",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");

    // Pre-install: created_template_ids is empty.
    let before: Json = client
        .get(server.api_url("/hub/assistants?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list before")
        .json()
        .await
        .expect("parse before");
    let pre = before
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "mock-asst-a")
        .expect("mock-asst-a must be in catalog");
    assert_eq!(
        pre["created_template_ids"].as_array().map(|a| a.len()),
        Some(0),
        "created_template_ids should start empty: {pre}",
    );

    let install: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .json()
        .await
        .expect("parse install");
    let installed_id = install["assistant"]["id"].as_str().unwrap().to_string();

    let after: Json = client
        .get(server.api_url("/hub/assistants?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after")
        .json()
        .await
        .expect("parse after");
    let post = after
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "mock-asst-a")
        .expect("mock-asst-a must still be in catalog");
    let ids: Vec<String> = post["created_template_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&installed_id),
        "created_template_ids must contain the new template uuid: {ids:?}",
    );
}

#[tokio::test]
async fn user_install_rejects_replace_existing_flag() {
    // `replace_existing` is template-only — passing it on the
    // user-scoped endpoint must 400 (not silently ignored), so
    // clients don't expect idempotent re-install behavior the
    // user path doesn't provide.
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
        ],
    )
    .await;
    let client = reqwest::Client::new();

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
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a", "replace_existing": true }))
        .send()
        .await
        .expect("install");
    assert_eq!(
        resp.status(),
        400,
        "replace_existing on user endpoint should 400, got {}",
        resp.status(),
    );
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["error_code"], "VALIDATION_ERROR");
}
