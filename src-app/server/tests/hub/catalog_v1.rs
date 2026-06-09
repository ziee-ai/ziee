//! Phase 1 — integration coverage for the unified hub catalog endpoints.
//!
//! Exercises the 5 new endpoints added in Phase 1:
//!   - GET    /api/hub/index
//!   - GET    /api/hub/version
//!   - POST   /api/hub/refresh    (admin)
//!   - GET    /api/hub/installed  (any auth; admin sees system rows too)
//!   - GET    /api/hub/manifest/:id?category=...
//!
//! The seed catalog (v0.0.1-alpha) is install-on-boot via include_dir!,
//! so every test starts with a populated catalog and doesn't need to
//! hit GitHub.

use serde_json::Value as Json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

// =====================================================================
// /hub/version + /hub/index — anyone with read can call
// =====================================================================

#[tokio::test]
async fn version_endpoint_returns_seed_catalog_metadata() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/hub/version"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send /hub/version");
    assert_eq!(response.status(), 200, "expected 200 for /hub/version");
    let body: Json = response.json().await.expect("parse json");
    assert_eq!(body["hub_version"], "0.0.3-alpha");
    let server_version = body["server_version"]
        .as_str()
        .expect("server_version is a string");
    assert!(
        !server_version.is_empty(),
        "server_version should be set: {body}"
    );
    let counts = &body["counts"];
    assert_eq!(counts["models"], 7);
    assert_eq!(counts["assistants"], 5);
    assert_eq!(counts["mcp_servers"], 6);
}

#[tokio::test]
async fn index_endpoint_lists_seed_items() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send /hub/index");
    assert_eq!(response.status(), 200);
    let catalog: Json = response.json().await.expect("parse json");
    assert_eq!(catalog["schema_version"], 1);
    assert_eq!(catalog["hub_version"], "0.0.3-alpha");
    let items = catalog["items"]
        .as_array()
        .expect("items should be an array");
    assert_eq!(items.len(), 18, "seed catalog has 18 items");

    // Spot-check known ids — the seed staging is fixed at v0.0.3-alpha.
    let ids: Vec<&str> = items.iter().filter_map(|i| i["id"].as_str()).collect();
    assert!(ids.contains(&"code-reviewer"), "missing code-reviewer in {ids:?}");
    assert!(ids.contains(&"llama-3-1-8b-instruct"));
    assert!(ids.contains(&"github-mcp"));
}

#[tokio::test]
async fn index_endpoint_requires_auth() {
    let server = TestServer::start().await;
    let no_perm = create_user_with_no_permissions(&server, "regular").await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", no_perm.token))
        .send()
        .await
        .expect("send /hub/index without perms");
    assert_eq!(
        response.status(),
        403,
        "non-permissioned user should be 403'd"
    );
}

// =====================================================================
// /hub/manifest/:id?category=... — per-id YAML reads
// =====================================================================

#[tokio::test]
async fn manifest_endpoint_returns_model_yaml() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/manifest/llama-3-1-8b-instruct?category=model"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send manifest GET");
    assert_eq!(response.status(), 200);
    let payload: Json = response.json().await.expect("parse json");
    // HubManifest is a typed struct: { category, model?, assistant?, mcp_server? }.
    assert_eq!(payload["category"], "model");
    assert_eq!(payload["model"]["id"], "llama-3-1-8b-instruct");
    assert_eq!(payload["model"]["file_format"], "safetensors");
    assert!(
        payload["assistant"].is_null() && payload["mcp_server"].is_null(),
        "only the model variant should be populated: {payload}"
    );
}

#[tokio::test]
async fn index_endpoint_401_without_token() {
    // Unauthenticated (no Bearer) → 401, distinct from the 403 a
    // wrong-permission user gets.
    let server = TestServer::start().await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/index"))
        .send()
        .await
        .expect("send /hub/index unauthenticated");
    assert_eq!(response.status(), 401, "missing token should be 401");
}

#[tokio::test]
async fn catalog_read_cannot_activate() {
    // The read/manage split: a user with only hub::catalog::read can
    // list releases + updates but NOT refresh/activate (manage).
    let server = TestServer::start().await;
    let reader =
        create_user_with_permissions(&server, "catreader", &["hub::catalog::read"]).await;
    let client = reqwest::Client::new();

    // read endpoints OK — /hub/installed is per-user; an admin who
    // can read the catalog also sees system-wide installs.
    let releases = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("installed");
    assert_eq!(releases.status(), 200, "catalog::read may view installed list");

    // manage endpoints forbidden
    let refresh = client
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("refresh");
    assert_eq!(refresh.status(), 403, "catalog::read may NOT refresh");

    let activate = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&serde_json::json!({ "version": "0.0.1-alpha" }))
        .send()
        .await
        .expect("activate");
    assert_eq!(activate.status(), 403, "catalog::read may NOT activate");
}

#[tokio::test]
async fn manifest_endpoint_404s_unknown_id() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/manifest/does-not-exist?category=model"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send manifest GET unknown");
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn manifest_endpoint_400s_unsafe_id() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    // Path-traversal attempt — URL encoding `..` so it survives axum routing.
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/manifest/..%2Fetc%2Fpasswd?category=model"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send unsafe id");
    assert!(
        response.status() == 400 || response.status() == 404,
        "expected 400/404 for traversal, got {}",
        response.status()
    );
}

// =====================================================================
// /hub/refresh — admin only
// =====================================================================

#[tokio::test]
async fn refresh_endpoint_requires_admin() {
    let server = TestServer::start().await;
    let reader = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("send refresh");
    assert_eq!(
        response.status(),
        403,
        "non-admin user should be 403'd from /hub/refresh"
    );
}

// =====================================================================
// /hub/installed — per-user view; admins (hub::catalog::read) ALSO see
// system-wide installs. Replaces the old admin-only /hub/updates.
// =====================================================================

#[tokio::test]
async fn installed_endpoint_open_to_any_authenticated_user() {
    // No permission gate beyond auth — every user has a personal view of
    // their own installs. Without an install of their own, the response is
    // an empty `items` array (system rows only become visible to admins).
    let server = TestServer::start().await;
    let reader = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("send installed");
    assert_eq!(response.status(), 200, "any authenticated user can view their installed list");
    let body: Json = response.json().await.expect("parse json");
    assert!(
        body["items"].as_array().expect("items array").is_empty(),
        "non-admin with no installs sees an empty list (no system rows leak): {body}"
    );
}

#[tokio::test]
async fn installed_endpoint_empty_when_no_installs() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("send installed as admin");
    assert_eq!(response.status(), 200);
    let body: Json = response.json().await.expect("parse json");
    assert_eq!(body["catalog_version"], "0.0.3-alpha");
    let items = body["items"].as_array().expect("items array");
    assert!(items.is_empty(), "no installs yet → empty list: {items:?}");
}

#[tokio::test]
async fn installed_endpoint_lists_all_tracked_entities() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;

    // Insert a synthetic system-wide hub_entities row (created_by NULL)
    // with an OLD hub_version. /hub/installed lists it regardless of
    // whether it matches the catalog; the row's `installed_version` vs
    // `current_version` is what the UI compares to flag staleness.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let entity_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO hub_entities (id, entity_type, entity_id, hub_id, hub_category, hub_version)
         VALUES ($1, 'assistant', $2, 'code-reviewer', 'assistant', '0.0.0-test')",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .execute(&pool)
    .await
    .expect("insert stale hub entity");
    pool.close().await;

    let response = reqwest::Client::new()
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("send installed as admin");
    assert_eq!(response.status(), 200);
    let body: Json = response.json().await.expect("parse json");
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1, "expected exactly one installed row, got {items:?}");
    assert_eq!(items[0]["hub_id"], "code-reviewer");
    assert_eq!(items[0]["installed_version"], "0.0.0-test");
    assert_eq!(items[0]["current_version"], "0.0.3-alpha");
    assert_eq!(items[0]["is_system"], true, "created_by NULL → is_system: {body}");
    assert!(items[0]["installed_at"].is_string(), "installed_at must be serialized: {body}");
}

// =====================================================================
// /hub/installed surfaces NULL hub_version (legacy rows pre-migration 69)
// =====================================================================

#[tokio::test]
async fn installed_endpoint_surfaces_null_version_rows() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    sqlx::query(
        "INSERT INTO hub_entities (id, entity_type, entity_id, hub_id, hub_category)
         VALUES ($1, 'mcp_server', $2, 'github-mcp', 'mcp_server')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect("insert NULL hub_version row");
    pool.close().await;

    let response = reqwest::Client::new()
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("send installed");
    let body: Json = response.json().await.expect("parse json");
    let items = body["items"].as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert!(
        items[0]["installed_version"].is_null(),
        "legacy row should have NULL installed_version: {body}"
    );
}

// =====================================================================
// /hub/releases + /hub/activate — admin version pinning
// =====================================================================

#[tokio::test]
async fn releases_endpoint_requires_admin() {
    let server = TestServer::start().await;
    let reader = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/releases"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("send releases");
    assert_eq!(
        response.status(),
        403,
        "non-admin user should be 403'd from /hub/releases"
    );
}

#[tokio::test]
async fn activate_endpoint_requires_admin() {
    let server = TestServer::start().await;
    let reader = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let response = reqwest::Client::new()
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&serde_json::json!({ "version": "0.0.1-alpha" }))
        .send()
        .await
        .expect("send activate");
    assert_eq!(
        response.status(),
        403,
        "non-admin user should be 403'd from /hub/activate"
    );
}

#[tokio::test]
async fn activate_rejects_unsafe_version() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;
    // Path-traversal-ish version string must be rejected before any
    // network fetch (400, not 500).
    let response = reqwest::Client::new()
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "version": "../../etc/passwd" }))
        .send()
        .await
        .expect("send activate unsafe");
    assert_eq!(
        response.status(),
        400,
        "unsafe version should be 400, got {}",
        response.status()
    );
}

// The following two hit the real ziee-ai/hub GitHub Releases. They
// assert the full pin → fetch → REAL cosign verify → rotate path across
// the published alpha versions — the one thing the hermetic mock can't
// cover (it skips cosign). #[ignore]'d so the default run stays
// network-free; run explicitly with `--ignored` to smoke the real
// signed releases.

#[tokio::test]
async fn releases_endpoint_lists_published_versions() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;
    let response = reqwest::Client::new()
        .get(server.api_url("/hub/releases"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("send releases");
    assert_eq!(response.status(), 200, "releases should 200 for admin");
    let body: Json = response.json().await.expect("parse json");
    // active_version is the seeded catalog until a refresh happens.
    assert_eq!(body["active_version"], "0.0.1-alpha");
    assert!(body["pinned_version"].is_null(), "no pin by default: {body}");
    let versions: Vec<&str> = body["releases"]
        .as_array()
        .expect("releases array")
        .iter()
        .filter_map(|r| r["version"].as_str())
        .collect();
    assert!(
        versions.contains(&"0.0.1-alpha") && versions.contains(&"0.0.2-alpha"),
        "expected both alpha versions, got {versions:?}"
    );
}

#[tokio::test]
async fn activate_switches_catalog_server_wide() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", &["hub::catalog::read", "hub::catalog::manage"]).await;
    let client = reqwest::Client::new();

    // Seed install is v0.0.1-alpha (13 items). Activate v0.0.2-alpha.
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "version": "0.0.2-alpha" }))
        .send()
        .await
        .expect("activate 0.0.2");
    assert_eq!(
        resp.status(),
        200,
        "activate 0.0.2-alpha should succeed (cosign verified): {}",
        resp.text().await.unwrap_or_default()
    );
    let body: Json = resp.json().await.expect("parse activate json");
    assert_eq!(body["new_version"], "0.0.2-alpha");
    assert_eq!(body["cosign_verified"], true);

    // Catalog is now server-wide v0.0.2-alpha (18 items). A plain
    // reader sees it too.
    let reader = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("send index")
        .json()
        .await
        .expect("parse index");
    assert_eq!(idx["hub_version"], "0.0.2-alpha");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(18));

    // The pin is persisted + reflected in /releases.
    let rel: Json = client
        .get(server.api_url("/hub/releases"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("send releases")
        .json()
        .await
        .expect("parse releases");
    assert_eq!(rel["pinned_version"], "0.0.2-alpha");
    assert_eq!(rel["active_version"], "0.0.2-alpha");

    // Activate back to v0.0.1-alpha — catalog shrinks to 13 items.
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "version": "0.0.1-alpha" }))
        .send()
        .await
        .expect("activate 0.0.1");
    assert_eq!(resp.status(), 200);
    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("send index 2")
        .json()
        .await
        .expect("parse index 2");
    assert_eq!(idx["hub_version"], "0.0.1-alpha");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(13));
}
