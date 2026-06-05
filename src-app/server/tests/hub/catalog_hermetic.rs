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

/// Companion to `two_versions` for MCP-system install tests. Same
/// shape (two versions, newest-first, both prerelease so the mock
/// release-server's "tag" semantics line up) but includes an MCP
/// server entry that the new system-install tests can install.
/// Kept separate from `two_versions` so the existing assistant tests
/// don't break on item-count assertions.
fn mcp_versions() -> Vec<MockVersion> {
    vec![
        MockVersion {
            version: "9.9.2-test",
            prerelease: true,
            items: vec![
                MockItem {
                    category: "mcp-server",
                    id: "mock-mcp-a",
                    min_ziee_version: None,
                    extra_yaml: None,
                },
                MockItem {
                    category: "mcp-server",
                    id: "mock-mcp-future",
                    min_ziee_version: Some("99.0.0"),
                    extra_yaml: None,
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                id: "mock-mcp-a",
                min_ziee_version: None,
                extra_yaml: None,
            }],
        },
    ]
}

fn two_versions() -> Vec<MockVersion> {
    vec![
        // Newest-first (GitHub order). v9.9.2 adds an incompatible
        // assistant (min_ziee_version 99.0.0).
        MockVersion {
            version: "9.9.2-test",
            prerelease: true,
            items: vec![
                MockItem {
                    category: "model",
                    id: "mock-model-a",
                    min_ziee_version: None,
                    extra_yaml: None,
                },
                MockItem {
                    category: "assistant",
                    id: "mock-asst-a",
                    min_ziee_version: None,
                    extra_yaml: None,
                },
                MockItem {
                    category: "assistant",
                    id: "mock-asst-future",
                    min_ziee_version: Some("99.0.0"),
                    extra_yaml: None,
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![
                MockItem {
                    category: "model",
                    id: "mock-model-a",
                    min_ziee_version: None,
                    extra_yaml: None,
                },
                MockItem {
                    category: "assistant",
                    id: "mock-asst-a",
                    min_ziee_version: None,
                    extra_yaml: None,
                },
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
    let third: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a", "replace_existing": true }))
        .send()
        .await
        .expect("third install")
        .json()
        .await
        .expect("parse third");
    let third_id = third["assistant"]["id"].as_str().unwrap().to_string();

    // After the third install, ONLY the third uuid should remain
    // (regression guard against a future change letting cleanup race
    // with the new track_hub_entity insert and leaving stale rows).
    let final_listing: Json = client
        .get(server.api_url("/hub/assistants?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after third")
        .json()
        .await
        .expect("parse final");
    let final_row = final_listing
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "mock-asst-a")
        .expect("mock-asst-a in final listing");
    let final_ids: Vec<String> = final_row["created_template_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        final_ids,
        vec![third_id.clone()],
        "after 3 replace installs, only the THIRD uuid should remain",
    );
    assert!(
        !final_ids.contains(&first_id) && !final_ids.contains(&second_id),
        "1st + 2nd uuids must be gone, got {final_ids:?}",
    );
}

#[tokio::test]
async fn replace_existing_preserves_is_default() {
    // Re-install via /hub/updates must not silently demote a previously
    // promoted template (is_default=true). Without this carry-forward,
    // re-install would set is_default=false (the request body default),
    // and new signups would stop receiving the template via the
    // clone-on-signup hook (which filters on is_default && enabled).
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

    // Install + promote to default in one shot (the API accepts the
    // is_default flag on install).
    let first: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a", "is_default": true }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    assert_eq!(first["assistant"]["is_default"], true);

    // Re-install WITHOUT passing is_default (mimics the
    // UpdatesHubTab Re-install button which passes only hub_id +
    // replace_existing). The new template must inherit is_default=true.
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
    assert_eq!(
        second["assistant"]["is_default"], true,
        "re-install must carry forward is_default=true: {second}",
    );
}

#[tokio::test]
async fn replace_existing_aborts_on_validation_failure() {
    // The `replace_existing` re-install path runs validation BEFORE
    // the delete so a failed re-install (e.g. catalog moved an item
    // to an incompatible min_ziee_version) doesn't leave the admin
    // with the prior template wiped and no system-wide fallback.
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

    // v9.9.1 has `mock-asst-a` compatible — install fine.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate v1")
        .error_for_status()
        .expect("activate v1 ok");
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

    // Activate v9.9.2 — same `mock-asst-a` is still present and
    // compatible, but we'll attempt `replace_existing` with an
    // explicit overflow on description to force a 400.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate v2")
        .error_for_status()
        .expect("activate v2 ok");

    // Oversized description (>4 KiB) triggers
    // validate_assistant_text_lengths → 400. The prior template must
    // still exist after this failure (delete is gated on validation).
    let oversized = "a".repeat(5000);
    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "hub_id": "mock-asst-a",
            "replace_existing": true,
            "description": oversized,
        }))
        .send()
        .await
        .expect("replace with bad input");
    assert_eq!(
        resp.status(),
        400,
        "validation failure should 400, got {}",
        resp.status(),
    );

    // The prior template MUST still exist in the catalog listing.
    let listing: Json = client
        .get(server.api_url("/hub/assistants?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after failed replace")
        .json()
        .await
        .expect("parse listing");
    let row = listing
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "mock-asst-a")
        .expect("mock-asst-a in catalog");
    let ids: Vec<String> = row["created_template_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&first_id),
        "prior template MUST survive a failed re-install: {ids:?}",
    );
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

    // Stamp a synthetic outdated MODEL row (also `created_by: NULL`)
    // and assert that `is_template_install` stays FALSE for it — the
    // predicate must filter on `entity_type == 'assistant'`, not just
    // `created_by IS NULL`. Without this guard, a hub model in /hub/
    // updates would route through the template re-install endpoint
    // (which would 404 since the model's hub_id isn't in the
    // assistants table). Inserts directly into `hub_entities` because
    // a real model install requires a provider + a real download.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test pool");
    sqlx::query!(
        r#"
        INSERT INTO hub_entities
            (entity_type, entity_id, hub_id, hub_category, created_by, hub_version)
        VALUES
            ('llm_model', gen_random_uuid(), 'mock-model-a', 'model', NULL, '9.9.1-test')
        "#,
    )
    .execute(&pool)
    .await
    .expect("insert synthetic model row");

    let updates2: Json = client
        .get(server.api_url("/hub/updates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("updates 2")
        .json()
        .await
        .expect("parse updates 2");
    let model_row = updates2["updates"]
        .as_array()
        .expect("updates2 array")
        .iter()
        .find(|r| r["hub_id"] == "mock-model-a")
        .expect("synthetic model row should appear in updates");
    assert_eq!(
        model_row["is_template_install"], false,
        "MODEL install must NOT be flagged as template even though \
         created_by IS NULL: {model_row}",
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

// ============================================================================
// /hub/mcp-servers/create-system — system-wide MCP server install
// ============================================================================
//
// Companion endpoint to /hub/mcp-servers/create. Same hub-load + verify +
// track flow, but creates a SYSTEM server (`is_system=true, user_id=NULL`,
// enforced by the `system_server_must_have_no_owner` CHECK constraint in
// migration 7) gated on `hub::mcp_servers::create + mcp_servers_admin::create`.

#[tokio::test]
async fn install_as_system_mcp_creates_server_with_null_owner() {
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("create system");
    assert_eq!(
        resp.status(),
        201,
        "system install should succeed: {}",
        resp.text().await.unwrap_or_default()
    );
    let body: Json = resp.json().await.expect("parse body");
    assert_eq!(
        body["server"]["is_system"], true,
        "is_system must be true: {body}",
    );
    assert!(
        body["server"]["user_id"].is_null(),
        "user_id must be null for system servers: {body}",
    );
    assert!(
        body["hub_tracking"]["created_by"].is_null(),
        "hub_tracking.created_by must be null for system installs: {body}",
    );
}

#[tokio::test]
async fn install_as_system_mcp_requires_admin_permission() {
    // User has hub::mcp_servers::create but NOT
    // mcp_servers_admin::create — endpoint requires BOTH, so 403.
    let mock = spawn_mock_hub(mcp_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let user_no_admin = create_user_with_permissions(
        &server,
        "user_no_admin",
        &["hub::catalog::read", "hub::mcp_servers::create"],
    )
    .await;
    let client = reqwest::Client::new();

    let resp = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header(
            "Authorization",
            format!("Bearer {}", user_no_admin.token),
        )
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("create system no perm");

    assert_eq!(
        resp.status(),
        403,
        "missing mcp_servers_admin::create → 403"
    );
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn install_as_system_mcp_duplicate_is_409() {
    // Idempotency guard: a second install with the same hub_id and
    // no `replace_existing` flag must 409 — otherwise the admin
    // accidentally creates duplicate system servers.
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("first install");
    assert_eq!(first.status(), 201);

    // Second install (no replace_existing) → 409.
    let second = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("second install");
    assert_eq!(
        second.status(),
        409,
        "duplicate system install should 409: {}",
        second.text().await.unwrap_or_default(),
    );
    let body: Json = second.json().await.unwrap();
    assert_eq!(body["error_code"], "RESOURCE_CONFLICT");
}

#[tokio::test]
async fn install_as_system_mcp_with_replace_existing_succeeds() {
    // `replace_existing: true` deletes the prior system server +
    // creates afresh. Cleanup is event-driven via
    // `CleanupHubEntitiesHandler` so the dangling hub_entities row
    // is also removed before the new track_hub_entity runs.
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::read",
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();

    let second: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a", "replace_existing": true }))
        .send()
        .await
        .expect("replace install")
        .json()
        .await
        .expect("parse second");
    let second_id = second["server"]["id"].as_str().unwrap().to_string();
    assert_ne!(first_id, second_id, "replace must produce a new uuid");

    // The prior server is gone — `created_system_ids` on the hub
    // listing reflects only the NEW id (INNER JOIN against
    // mcp_servers in get_system_mcp_install_ids self-cleans
    // deletions, and the CleanupHubEntitiesHandler removes the
    // dangling hub_entities row event-driven from the
    // McpServerEvent::SystemServerDeleted emit).
    let listing: Json = client
        .get(server.api_url("/hub/mcp-servers?lang=en"))
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
        .find(|s| s["id"] == "mock-mcp-a")
        .expect("mock-mcp-a in listing");
    let ids: Vec<String> = row["created_system_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![second_id.clone()], "only NEW id should remain");
    assert!(
        !ids.contains(&first_id),
        "old id must be gone from created_system_ids",
    );

    // A THIRD `replace_existing` install should also succeed — the
    // find_system_mcp_install JOIN guards against orphan rows.
    let third: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a", "replace_existing": true }))
        .send()
        .await
        .expect("third install")
        .json()
        .await
        .expect("parse third");
    let third_id = third["server"]["id"].as_str().unwrap().to_string();

    // After 3 installs, only the third uuid should remain.
    let final_listing: Json = client
        .get(server.api_url("/hub/mcp-servers?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after third")
        .json()
        .await
        .expect("parse final");
    let final_row = final_listing
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "mock-mcp-a")
        .expect("mock-mcp-a in final listing");
    let final_ids: Vec<String> = final_row["created_system_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        final_ids,
        vec![third_id.clone()],
        "after 3 replace installs, only the THIRD uuid should remain",
    );
}

#[tokio::test]
async fn system_mcp_install_surfaces_in_updates_with_system_flag() {
    // When the catalog version moves forward AFTER a system install,
    // /hub/updates surfaces the row with `is_system_mcp_install: true`
    // (and `is_template_install: false` — the flags must coexist
    // without crossing predicates) so the UI routes the Re-install
    // action through the system MCP endpoint instead of the
    // user-install endpoint (which would silently demote the system
    // server to a personal one).
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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
        .expect("activate v1")
        .error_for_status()
        .expect("activate v1 ok");
    client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
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
        .find(|r| r["hub_id"] == "mock-mcp-a")
        .expect("system install must appear in updates");
    assert_eq!(
        row["is_system_mcp_install"], true,
        "system MCP install must be flagged: {row}",
    );
    assert_eq!(
        row["is_template_install"], false,
        "the system MCP row must NOT be flagged as template: {row}",
    );
}

#[tokio::test]
async fn replace_existing_aborts_on_validation_failure_mcp() {
    // MCP analog of `replace_existing_aborts_on_validation_failure`
    // (the template test). The `replace_existing` re-install path
    // runs the helper (which calls `ensure_installable` +
    // `validate_transport_config`) BEFORE the delete, so a failed
    // re-install — e.g. the upstream maintainer raised
    // `min_ziee_version` past the server — must NOT leave the admin
    // with the prior system MCP server wiped.
    //
    // MCP requests don't accept transport / command / url overrides,
    // so we trigger validation failure via `ensure_installable`
    // by activating a version where `mock-mcp-a` is pinned to
    // min_ziee_version > server. The mock-mcp-a in the v9.9.1 ARM
    // is compatible (install fine); the v9.9.4 arm makes the same
    // hub_id incompatible.
    let mock = spawn_mock_hub(vec![
        MockVersion {
            version: "9.9.4-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                id: "mock-mcp-a",
                min_ziee_version: Some("99.0.0"),
                extra_yaml: None,
            }],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                id: "mock-mcp-a",
                min_ziee_version: None,
                extra_yaml: None,
            }],
        },
    ])
    .await;
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
            "hub::mcp_servers::read",
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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
        .expect("activate v1")
        .error_for_status()
        .expect("activate v1 ok");

    let first: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();

    // Activate v9.9.4 — same `mock-mcp-a` is now incompatible.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.4-test" }))
        .send()
        .await
        .expect("activate v2")
        .error_for_status()
        .expect("activate v2 ok");

    // Attempt re-install — ensure_installable fires → 422.
    let resp = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "hub_id": "mock-mcp-a",
            "replace_existing": true,
        }))
        .send()
        .await
        .expect("replace with incompatible");
    assert_eq!(
        resp.status(),
        422,
        "incompatible re-install should 422, got {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default(),
    );

    // Prior server MUST survive — it's still in `created_system_ids`.
    let listing: Json = client
        .get(server.api_url("/hub/mcp-servers?lang=en"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list after failed replace")
        .json()
        .await
        .expect("parse listing");
    let row = listing
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "mock-mcp-a")
        .expect("mock-mcp-a in catalog");
    let ids: Vec<String> = row["created_system_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&first_id),
        "prior system server MUST survive a failed re-install: {ids:?}",
    );
}

#[tokio::test]
async fn user_mcp_install_rejects_replace_existing_flag() {
    // `replace_existing` is system-only — passing it on the
    // user-scoped MCP endpoint must 400, not silently ignored.
    // Mirrors `user_install_rejects_replace_existing_flag` for
    // assistants.
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::create",
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
        .post(server.api_url("/hub/mcp-servers/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a", "replace_existing": true }))
        .send()
        .await
        .expect("install");
    assert_eq!(
        resp.status(),
        400,
        "replace_existing on user MCP endpoint should 400, got {}",
        resp.status(),
    );
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["error_code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn replace_existing_preserves_admin_tunable_fields_mcp() {
    // Re-install via /hub/updates must not silently demote a
    // previously-promoted system MCP server's admin-tunable runtime
    // fields. Sibling of `replace_existing_preserves_is_default`.
    // Specifically guards against the Round-1 F-1 silent-demote on
    // Re-install for ALL FIVE carried-forward fields — single test
    // catches a regression on any of the lines at once instead of
    // one test per field (`run_in_sandbox` alone would let a future
    // refactor silently drop `usage_mode` carry-forward unnoticed).
    //
    // We install, then PUT the row via the native admin endpoint to
    // promote all five fields away from the hub defaults, then
    // re-install and assert every field survives.
    let mock = spawn_mock_hub(mcp_versions()).await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a" }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();
    // Sanity: hub install starts with the catalog/code defaults.
    assert_eq!(
        first["server"]["run_in_sandbox"], false,
        "hub install must start with run_in_sandbox=false",
    );
    assert_eq!(
        first["server"]["enabled"], true,
        "hub install must start with enabled=true",
    );

    // Promote ALL SEVEN admin-tunable fields away from defaults via
    // native admin PUT. Note: `enabled=false` exercises the
    // enabled-stays-disabled regression guard; `usage_mode=always`
    // and the session cap + timeout exercise three more.
    // `environment_variables` + `headers` exercise the
    // required-input carry-forward (real values pasted by the admin
    // must survive Re-install, not get stomped back to placeholders).
    let promote = client
        .put(server.api_url(&format!("/mcp/system-servers/{}", first_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "run_in_sandbox": true,
            "enabled": false,
            "usage_mode": "always",
            "max_concurrent_sessions": 7,
            "timeout_seconds": 120,
            "environment_variables": {
                "ADMIN_SET_KEY": "real_value_pasted_by_admin"
            },
            "headers": {
                "X-Admin-Set-Header": "real_header_value"
            }
        }))
        .send()
        .await
        .expect("promote");
    assert_eq!(
        promote.status(),
        200,
        "PUT should succeed: {}",
        promote.text().await.unwrap_or_default(),
    );

    // Re-install without passing any of these in the request
    // (mimics UpdatesHubTab.reinstall which only passes hub_id +
    // replace_existing). All seven fields must carry forward.
    let second: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-a", "replace_existing": true }))
        .send()
        .await
        .expect("replace")
        .json()
        .await
        .expect("parse second");
    assert_eq!(
        second["server"]["run_in_sandbox"], true,
        "re-install must carry forward run_in_sandbox=true: {second}",
    );
    assert_eq!(
        second["server"]["enabled"], false,
        "re-install must carry forward enabled=false: {second}",
    );
    assert_eq!(
        second["server"]["usage_mode"], "always",
        "re-install must carry forward usage_mode=always: {second}",
    );
    assert_eq!(
        second["server"]["max_concurrent_sessions"], 7,
        "re-install must carry forward max_concurrent_sessions=7: {second}",
    );
    assert_eq!(
        second["server"]["timeout_seconds"], 120,
        "re-install must carry forward timeout_seconds=120: {second}",
    );
    assert_eq!(
        second["server"]["environment_variables"]["ADMIN_SET_KEY"],
        "real_value_pasted_by_admin",
        "re-install must carry forward env vars set by admin: {second}",
    );
    assert_eq!(
        second["server"]["headers"]["X-Admin-Set-Header"],
        "real_header_value",
        "re-install must carry forward headers set by admin: {second}",
    );
}

// ============================================================================
// Required-input schema — placeholder seeding on install
// ============================================================================
//
// The hub schema gained `required_env` + `required_headers` lists declaring
// inputs the user must configure. The install path seeds the new MCP row's
// env / header maps from each required input's `placeholder` so the user
// sees in the settings page exactly what to replace (instead of an opaque
// empty string).

#[tokio::test]
async fn install_with_required_inputs_seeds_placeholders_mcp() {
    // Mock catalog with one MCP server declaring one required env var
    // and one required header, each with a recognizable placeholder.
    // After install, both placeholders must land in the new server
    // row's `environment_variables` / `headers` maps verbatim.
    let mock = spawn_mock_hub(vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem {
            category: "mcp-server",
            id: "mock-mcp-needs-config",
            min_ziee_version: None,
            extra_yaml: Some(
                "required_env:\n\
                 - name: MOCK_API_KEY\n\
                 \x20\x20description: Mock service API key\n\
                 \x20\x20placeholder: mk_xxxxxxxxxxxxxxxxxxxx\n\
                 \x20\x20is_secret: true\n\
                 required_headers:\n\
                 - name: X-Mock-Tenant\n\
                 \x20\x20description: Mock tenant identifier\n\
                 \x20\x20placeholder: tenant_abc123\n\
                 \x20\x20is_secret: false\n",
            ),
        }],
    }])
    .await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
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

    let resp: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-needs-config" }))
        .send()
        .await
        .expect("install")
        .json()
        .await
        .expect("parse install");

    assert_eq!(
        resp["server"]["environment_variables"]["MOCK_API_KEY"],
        "mk_xxxxxxxxxxxxxxxxxxxx",
        "install must seed env-var placeholder verbatim: {resp}",
    );
    assert_eq!(
        resp["server"]["headers"]["X-Mock-Tenant"],
        "tenant_abc123",
        "install must seed header placeholder verbatim: {resp}",
    );
}

#[tokio::test]
async fn replace_existing_preserves_env_var_overrides_mcp() {
    // Install with placeholder, edit env var to a real value, then
    // re-install with `replace_existing: true`. The real value must
    // survive — otherwise Re-install silently breaks the server by
    // stomping the admin's real token with the placeholder again.
    let mock = spawn_mock_hub(vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem {
            category: "mcp-server",
            id: "mock-mcp-needs-key",
            min_ziee_version: None,
            extra_yaml: Some(
                "required_env:\n\
                 - name: API_KEY\n\
                 \x20\x20placeholder: placeholder_value\n\
                 \x20\x20is_secret: true\n",
            ),
        }],
    }])
    .await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-needs-key" }))
        .send()
        .await
        .expect("install")
        .json()
        .await
        .expect("parse install");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();

    // Admin pastes the real value.
    let promote = client
        .put(server.api_url(&format!("/mcp/system-servers/{}", first_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "environment_variables": {
                "API_KEY": "real_secret_token_admin_pasted"
            }
        }))
        .send()
        .await
        .expect("promote");
    assert_eq!(promote.status(), 200);

    let second: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-needs-key", "replace_existing": true }))
        .send()
        .await
        .expect("replace")
        .json()
        .await
        .expect("parse replace");

    assert_eq!(
        second["server"]["environment_variables"]["API_KEY"],
        "real_secret_token_admin_pasted",
        "re-install MUST carry forward the admin's real value, not \
         stomp it back to the placeholder: {second}",
    );
}

#[tokio::test]
async fn replace_existing_preserves_header_overrides_mcp() {
    // Header carry-forward symmetric to the env-var test: install,
    // PUT a header to a real value, re-install, assert the header
    // survives. Covers the `required_headers` path specifically.
    let mock = spawn_mock_hub(vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem {
            category: "mcp-server",
            id: "mock-mcp-needs-header",
            min_ziee_version: None,
            extra_yaml: Some(
                "required_headers:\n\
                 - name: X-Tenant-ID\n\
                 \x20\x20placeholder: tenant_placeholder\n\
                 \x20\x20is_secret: false\n",
            ),
        }],
    }])
    .await;
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
            "hub::mcp_servers::create",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
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
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-needs-header" }))
        .send()
        .await
        .expect("install")
        .json()
        .await
        .expect("parse install");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();

    let promote = client
        .put(server.api_url(&format!("/mcp/system-servers/{}", first_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "headers": {
                "X-Tenant-ID": "tenant_real_id_admin_pasted"
            }
        }))
        .send()
        .await
        .expect("promote");
    assert_eq!(promote.status(), 200);

    let second: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-mcp-needs-header", "replace_existing": true }))
        .send()
        .await
        .expect("replace")
        .json()
        .await
        .expect("parse replace");

    assert_eq!(
        second["server"]["headers"]["X-Tenant-ID"],
        "tenant_real_id_admin_pasted",
        "re-install MUST carry forward header values set by admin: {second}",
    );
}
