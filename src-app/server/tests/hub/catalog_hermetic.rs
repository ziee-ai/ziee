//! Hermetic hub catalog tests — no network.
//!
//! Uses the in-test `mock_release_server` (a mini Pages site over
//! loopback) + the debug-only `ZIEE_HUB_PAGES_BASE` override so the
//! full refresh → parse-index → lazy-fetch-manifest path is exercised
//! against a local server. There is no "activate by tag" or cosign
//! chain; the flow is:
//!
//!   mock.switch_to(version)           // publisher updates the catalog
//!   POST /hub/refresh (admin)         // server pulls the new index
//!
//! Helper [`apply_catalog`] below performs both in one call so the
//! test bodies stay readable.

use serde_json::{json, Value as Json};

use super::mock_release_server::{spawn_mock_hub, MockHub, MockItem, MockVersion};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// Find the first entry in a JSON `environment_variables_entries` or
/// `headers_entries` array whose `key` field matches `name`. Used by
/// the replace-existing tests that assert per-entry value/secrecy
/// after a re-install.
fn find_entry<'a>(entries: &'a Json, name: &str) -> &'a Json {
    entries
        .as_array()
        .unwrap_or_else(|| panic!("entries should be a JSON array, got: {entries}"))
        .iter()
        .find(|e| e.get("key").and_then(|v| v.as_str()) == Some(name))
        .unwrap_or_else(|| {
            panic!("entry with key {name} not found in: {entries}")
        })
}

/// Switch the mock's published catalog to `version` and force the
/// server to pull it via `/hub/refresh`. The catalog is in-place at
/// the Pages base + refresh is the only knob (no per-version
/// pinning). Asserting 200 here keeps the test failure message
/// focused on the code-under-test, not on a missed setup step.
async fn apply_catalog(
    mock: &MockHub,
    server: &TestServer,
    admin_token: &str,
    version: &str,
) {
    mock.switch_to(version);
    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .expect("refresh");
    assert_eq!(
        resp.status(),
        200,
        "/hub/refresh against mock Pages must 200; got {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default(),
    );
}

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
                    name: "io.github.test/mock-mcp-a",
                    min_ziee_version: None,
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
                MockItem {
                    category: "mcp-server",
                    name: "io.github.test/mock-mcp-future",
                    min_ziee_version: Some("99.0.0"),
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                name: "io.github.test/mock-mcp-a",
                min_ziee_version: None,
                extra_json: None,
                mcp_http: false,
                bundle_files: None,
                bundle_entry_point: None,
            }],
        },
    ]
}

/// `mcp_versions()` mirror that ships HTTP-transport manifests
/// instead of stdio. Use when the test installs the MCP through the
/// user-scoped endpoint (`/hub/mcp-servers/create`), which the MCP
/// user policy gates against stdio when `code_sandbox.enabled` is
/// false (the test default). System/admin installs still use the
/// stdio version since the user-policy gate doesn't apply there.
fn mcp_versions_http() -> Vec<MockVersion> {
    vec![
        MockVersion {
            version: "9.9.2-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                name: "io.github.test/mock-mcp-a",
                min_ziee_version: None,
                extra_json: None,
                mcp_http: true,
                bundle_files: None,
                bundle_entry_point: None,
            }],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                name: "io.github.test/mock-mcp-a",
                min_ziee_version: None,
                extra_json: None,
                mcp_http: true,
                bundle_files: None,
                bundle_entry_point: None,
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
                    name: "io.github.test/mock-model-a",
                    min_ziee_version: None,
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
                MockItem {
                    category: "assistant",
                    name: "io.github.test/mock-asst-a",
                    min_ziee_version: None,
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
                MockItem {
                    category: "assistant",
                    name: "io.github.test/mock-asst-future",
                    min_ziee_version: Some("99.0.0"),
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![
                MockItem {
                    category: "model",
                    name: "io.github.test/mock-model-a",
                    min_ziee_version: None,
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
                MockItem {
                    category: "assistant",
                    name: "io.github.test/mock-asst-a",
                    min_ziee_version: None,
                    extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
                },
            ],
        },
    ]
}

#[tokio::test]
async fn refresh_picks_up_publisher_catalog_changes() {
    // Covers the end-to-end refresh flow: a publisher updates
    // `index.json` on the Pages branch, an admin POSTs /hub/refresh,
    // the server pulls the new index in place. Tested by flipping the
    // mock's published catalog with `MockHub::switch_to` between two
    // refreshes.
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

    // Publisher state: older catalog (2 items). Refresh pulls it.
    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

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

    // /version reports source=pages (a Pages fetch replaced the seed).
    // v1's "github" provenance + cosign_verified field are gone.
    let ver: Json = client
        .get(server.api_url("/hub/version"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("version")
        .json()
        .await
        .expect("parse version");
    assert_eq!(ver["source"], "pages");
    assert_eq!(ver["hub_version"], "9.9.1-test");

    // Publisher pushes a newer catalog (3 items). Refresh picks it up
    // in place — no per-version pin, no rotation step.
    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;
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
    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;

    // A compatible assistant installs fine (201).
    let ok = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-asst-future" }))
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
async fn install_stamps_current_version_so_installed_row_is_not_outdated() {
    // Directly exercises the hub_version write-back: installing from the
    // active catalog must stamp hub_entities.hub_version with the current
    // version, so the row's `installed_version` matches `current_version`
    // (= not outdated). Regression guard for the bug where every install
    // was recorded with NULL and showed as needing an update.
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
    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;
    client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");

    // The fresh install was stamped 9.9.1-test == current → row exists
    // but with `installed_version == current_version`, so the UI's
    // outdated badge is not triggered.
    let installed: Json = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("installed")
        .json()
        .await
        .expect("parse installed");
    let rows = installed["items"].as_array().expect("items array");
    let row = rows
        .iter()
        .find(|r| r["hub_id"] == "io.github.test/mock-asst-a")
        .unwrap_or_else(|| panic!("fresh install must appear in /hub/installed: {installed}"));
    assert_eq!(
        row["installed_version"], row["current_version"],
        "freshly-installed item must be stamped at the current catalog version: {row}"
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
    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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

    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;

    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-future" }))
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    // First install succeeds.
    let first = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
        .send()
        .await
        .expect("first install");
    assert_eq!(first.status(), 201);

    // Second install (no replace_existing) → 409.
    let second = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let first: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a", "replace_existing": true }))
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
        .find(|a| a["name"] == "io.github.test/mock-asst-a")
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
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a", "replace_existing": true }))
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
        .find(|a| a["name"] == "io.github.test/mock-asst-a")
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
    // Re-install via /hub/installed must not silently demote a previously
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    // Install + promote to default in one shot (the API accepts the
    // is_default flag on install).
    let first: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a", "is_default": true }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a", "replace_existing": true }))
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
    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;
    let first: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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
    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;

    // Oversized description (>4 KiB) triggers
    // validate_assistant_text_lengths → 400. The prior template must
    // still exist after this failure (delete is gated on validation).
    let oversized = "a".repeat(5000);
    let resp = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "hub_id": "io.github.test/mock-asst-a",
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
        .find(|a| a["name"] == "io.github.test/mock-asst-a")
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
async fn template_install_surfaces_in_installed_with_template_flag() {
    // When the catalog version moves forward AFTER a template install,
    // /hub/installed surfaces the row with `is_template_install: true`
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
    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;
    client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");
    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;

    let installed: Json = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("installed")
        .json()
        .await
        .expect("parse installed");
    let row = installed["items"]
        .as_array()
        .expect("items array")
        .iter()
        .find(|r| r["hub_id"] == "io.github.test/mock-asst-a")
        .expect("template install must appear in /hub/installed");
    assert_eq!(
        row["is_template_install"], true,
        "template install must be flagged: {row}",
    );
    assert_ne!(
        row["installed_version"], row["current_version"],
        "row must surface as outdated after the v9.9.2 activate: {row}",
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
            ('llm_model', gen_random_uuid(), 'io.github.test/mock-model-a', 'model', NULL, '9.9.1-test')
        "#,
    )
    .execute(&pool)
    .await
    .expect("insert synthetic model row");

    let installed2: Json = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("installed 2")
        .json()
        .await
        .expect("parse installed 2");
    let model_row = installed2["items"]
        .as_array()
        .expect("items2 array")
        .iter()
        .find(|r| r["hub_id"] == "io.github.test/mock-model-a")
        .expect("synthetic model row should appear in /hub/installed");
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

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
        .find(|a| a["name"] == "io.github.test/mock-asst-a")
        .expect("mock-asst-a must be in catalog");
    assert_eq!(
        pre["created_template_ids"].as_array().map(|a| a.len()),
        Some(0),
        "created_template_ids should start empty: {pre}",
    );

    let install: Json = client
        .post(server.api_url("/hub/assistant-templates/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a" }))
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
        .find(|a| a["name"] == "io.github.test/mock-asst-a")
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
async fn user_install_honors_replace_existing_flag() {
    // `replace_existing` IS honored on the user-scoped endpoint: it is the
    // Installed-tab "Re-install" path (hub PR #79), where the frontend posts
    // `replace_existing: true` to wipe the user's prior install of the same
    // hub_id before creating the fresh one. With no prior install it simply
    // creates (201). (This replaces the older "template-only → 400" contract,
    // which PR #75/#79 superseded.)
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let resp = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-asst-a", "replace_existing": true }))
        .send()
        .await
        .expect("install");
    assert_eq!(
        resp.status(),
        201,
        "replace_existing is honored on the user endpoint (Installed-tab \
         Re-install path); should create, got {}",
        resp.status(),
    );
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["assistant"]["id"].is_string(),
        "a successful install returns the created assistant: {body}"
    );
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let resp = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    // First install succeeds.
    let first = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
        .send()
        .await
        .expect("first install");
    assert_eq!(first.status(), 201);

    // Second install (no replace_existing) → 409.
    let second = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let first: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
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
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a", "replace_existing": true }))
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
        .find(|s| s["name"] == "io.github.test/mock-mcp-a")
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
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a", "replace_existing": true }))
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
        .find(|s| s["name"] == "io.github.test/mock-mcp-a")
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
async fn system_mcp_install_surfaces_in_installed_with_system_flag() {
    // When the catalog version moves forward AFTER a system install,
    // /hub/installed surfaces the row with `is_system_mcp_install: true`
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;
    client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");
    apply_catalog(&mock, &server, &admin.token, "9.9.2-test").await;

    let installed: Json = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("installed")
        .json()
        .await
        .expect("parse installed");
    let row = installed["items"]
        .as_array()
        .expect("items array")
        .iter()
        .find(|r| r["hub_id"] == "io.github.test/mock-mcp-a")
        .expect("system install must appear in /hub/installed");
    assert_eq!(
        row["is_system_mcp_install"], true,
        "system MCP install must be flagged: {row}",
    );
    assert_eq!(
        row["is_template_install"], false,
        "the system MCP row must NOT be flagged as template: {row}",
    );
    assert_ne!(
        row["installed_version"], row["current_version"],
        "after the v9.9.2 activate, the row must surface as outdated: {row}",
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
                name: "io.github.test/mock-mcp-a",
                min_ziee_version: Some("99.0.0"),
                extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
            }],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![MockItem {
                category: "mcp-server",
                name: "io.github.test/mock-mcp-a",
                min_ziee_version: None,
                extra_json: None,
                    mcp_http: false,
                    bundle_files: None,
                    bundle_entry_point: None,
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let first: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
        .send()
        .await
        .expect("first install")
        .json()
        .await
        .expect("parse first");
    let first_id = first["server"]["id"].as_str().unwrap().to_string();

    // Activate v9.9.4 — same `mock-mcp-a` is now incompatible.
    apply_catalog(&mock, &server, &admin.token, "9.9.4-test").await;

    // Attempt re-install — ensure_installable fires → 422.
    let resp = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "hub_id": "io.github.test/mock-mcp-a",
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
        .find(|s| s["name"] == "io.github.test/mock-mcp-a")
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
async fn user_mcp_install_honors_replace_existing_flag() {
    // `replace_existing` IS honored on the user-scoped MCP endpoint: it is the
    // Installed-tab "Re-install" path (hub PR #79), which posts
    // `replace_existing: true` to wipe the user's prior install before creating
    // the fresh one. With no prior install it simply creates (201). Mirrors
    // `user_install_honors_replace_existing_flag` for assistants.
    //
    // Uses the http-MCP mock catalog rather than the default stdio one
    // because the MCP user policy filters `'stdio'` out of
    // `allowed_transports` whenever `code_sandbox.enabled` is false
    // (test default), so a stdio user install would 422 with
    // `MCP_TRANSPORT_NOT_ALLOWED` before reaching the replace_existing path.
    let mock = spawn_mock_hub(mcp_versions_http()).await;
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let resp = client
        .post(server.api_url("/hub/mcp-servers/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a", "replace_existing": true }))
        .send()
        .await
        .expect("install");
    let status = resp.status();
    let body: Json = resp.json().await.unwrap();
    assert_eq!(
        status,
        201,
        "replace_existing is honored on the user MCP endpoint (Installed-tab \
         Re-install path); should create, got {} body: {}",
        status, body,
    );
    assert!(
        body["server"]["id"].is_string(),
        "a successful install returns the created MCP server: {body}"
    );
}

#[tokio::test]
async fn replace_existing_preserves_admin_tunable_fields_mcp() {
    // Re-install via /hub/installed must not silently demote a
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

    apply_catalog(&mock, &server, &admin.token, "9.9.1-test").await;

    let first: Json = client
        .post(server.api_url("/hub/mcp-servers/create-system"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a" }))
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
    // Hub installs now always start DISABLED (the user configures
    // secrets first, then toggles the Enabled switch which runs the
    // probe-then-enable flow). See
    // hub::handlers::build_mcp_server_create_from_hub.
    assert_eq!(
        first["server"]["enabled"], false,
        "hub install must start with enabled=false (user configures + enables manually)",
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
            "environment_variables_entries": [
                { "key": "ADMIN_SET_KEY", "value": "real_value_pasted_by_admin", "is_secret": false }
            ],
            "headers_entries": [
                { "key": "X-Admin-Set-Header", "value": "real_header_value", "is_secret": false }
            ]
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
        .json(&json!({ "hub_id": "io.github.test/mock-mcp-a", "replace_existing": true }))
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
    let env_entry = find_entry(
        &second["server"]["environment_variables_entries"],
        "ADMIN_SET_KEY",
    );
    assert_eq!(
        env_entry["is_secret"], false,
        "ADMIN_SET_KEY must stay non-secret: {second}",
    );
    assert_eq!(
        env_entry["value"], "real_value_pasted_by_admin",
        "re-install must carry forward env vars set by admin: {second}",
    );
    let hdr_entry = find_entry(
        &second["server"]["headers_entries"],
        "X-Admin-Set-Header",
    );
    assert_eq!(
        hdr_entry["is_secret"], false,
        "X-Admin-Set-Header must stay non-secret: {second}",
    );
    assert_eq!(
        hdr_entry["value"], "real_header_value",
        "re-install must carry forward headers set by admin: {second}",
    );
}

// ============================================================================
// REMOVED: required_env/required_headers placeholder seeding tests
// ============================================================================
// The legacy install path read `hub_mcp_server.required_env[*].placeholder`
// and seeded the new MCP server's env map with those values. The
// `required_env` + `required_headers` fields were dropped from
// `HubMCPServer` when the body moved to strict server.json (env vars now
// declared per-package in `packages[i].environmentVariables`, headers in
// `remotes[i].headers`). The four tests that exercised the old
// placeholder-seeding + replace-existing-merging behavior are deleted (not
// ignored — the feature is gone, see memory note
// `feedback_no_ignore_unless_platform`).
