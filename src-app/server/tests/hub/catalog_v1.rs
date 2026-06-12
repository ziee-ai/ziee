//! Phase 1 — integration coverage for the unified hub catalog endpoints
//! against the embedded v2 seed catalog.
//!
//! Exercises the surviving Hub v2 endpoints (v1's `/hub/releases` +
//! `/hub/activate` are gone — pinning was retired with the Pages
//! migration):
//!   - GET    /api/hub/index
//!   - GET    /api/hub/version
//!   - POST   /api/hub/refresh    (admin)
//!   - GET    /api/hub/installed  (any auth; admin sees system rows too)
//!   - GET    /api/hub/manifest/:id?category=...
//!
//! The v2 seed (hub_version `2.0.0`, 5 entries — 2 models, 1 assistant,
//! 2 MCP servers) is install-on-boot via `include_dir!`, so every test
//! starts with a populated catalog and doesn't need to hit GitHub.

use serde_json::Value as Json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

/// Hub-version constant of the embedded v2 seed (mirrors
/// `resources/hub-seed/index.json::hub_version`). Hard-coded rather
/// than loaded dynamically so a bumped seed forces test review.
const SEED_VERSION: &str = "2.0.0";
// The seed mirrors ziee-ai/hub's published `dist/` — 7 models +
// 5 assistants + 6 mcp-servers = 18 entries. Bump when the seed
// snapshot is refreshed.
const SEED_ITEM_COUNT: usize = 18;

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
    assert_eq!(body["hub_version"], SEED_VERSION);
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
    // schema_version is `2` under Hub v2. v1 callers were `1`; existing
    // v1-shaped JSON still deserializes because IndexItem has serde
    // defaults on the v2 envelope fields (qualified_name, version,
    // _meta), but the seed itself is freshly authored as v2 = 2.
    assert_eq!(catalog["schema_version"], 2);
    assert_eq!(catalog["hub_version"], SEED_VERSION);
    let items = catalog["items"]
        .as_array()
        .expect("items should be an array");
    assert_eq!(
        items.len(),
        SEED_ITEM_COUNT,
        "seed catalog has {SEED_ITEM_COUNT} items"
    );

    // Spot-check known ids — the seed is fixed at v2.0.0.
    // v2 IndexItem uses `name` (reverse-DNS) — `id` was dropped.
    let ids: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    assert!(ids.contains(&"io.github.phibya/code-reviewer"), "missing code-reviewer in {ids:?}");
    assert!(ids.contains(&"io.github.phibya/llama-3-1-8b-instruct"));
    assert!(ids.contains(&"io.github.github/mcp"));

    // v2 envelope additions: every seeded item ships a per-entry
    // `version` string (the source of truth for the per-row
    // `current_version` on `/hub/installed`).
    for item in items {
        let v = item["version"].as_str().expect("v2 items have a version");
        assert!(!v.is_empty(), "non-empty per-entry version: {item}");
    }
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
// /hub/manifest/:id?category=... — per-id JSON manifest reads
// =====================================================================

#[tokio::test]
async fn manifest_endpoint_returns_model_json() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    // v2 manifest lookup is by reverse-DNS `name` (URL-encoded `/`).
    let response = reqwest::Client::new()
        .get(server.api_url(
            "/hub/manifest/io.github.phibya%2Fllama-3-1-8b-instruct?category=model",
        ))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send manifest GET");
    assert_eq!(response.status(), 200);
    let payload: Json = response.json().await.expect("parse json");
    // HubManifest is a typed struct: { category, model?, assistant?, mcp_server? }.
    assert_eq!(payload["category"], "model");
    assert_eq!(payload["model"]["name"], "io.github.phibya/llama-3-1-8b-instruct");
    // v2 Phase 7 dropped the model-wide `file_format`; check the first source.
    assert_eq!(payload["model"]["sources"][0]["fileFormat"], "safetensors");
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
async fn catalog_read_cannot_refresh() {
    // The read/manage split: a user with only hub::catalog::read can
    // view installed + manifest endpoints but NOT refresh (manage).
    // v1's `/hub/activate` is gone — the manage-perm check is now only
    // gating `/hub/refresh`.
    let server = TestServer::start().await;
    let reader =
        create_user_with_permissions(&server, "catreader", &["hub::catalog::read"]).await;
    let client = reqwest::Client::new();

    // read endpoints OK — /hub/installed is per-user; an admin who
    // can read the catalog also sees system-wide installs.
    let installed = client
        .get(server.api_url("/hub/installed"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("installed");
    assert_eq!(installed.status(), 200, "catalog::read may view installed list");

    // manage endpoint forbidden
    let refresh = client
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .expect("refresh");
    assert_eq!(refresh.status(), 403, "catalog::read may NOT refresh");
}

#[tokio::test]
async fn manifest_endpoint_404s_unknown_id() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "reader", &["hub::models::read"]).await;
    // v2 manifest lookup is by reverse-DNS `name`. A well-formed-but-
    // unknown name should return 404; the bare slug `does-not-exist`
    // would be rejected by `is_safe_name` as 400 (covered by
    // `manifest_endpoint_400s_unsafe_id` below).
    let response = reqwest::Client::new()
        .get(server.api_url(
            "/hub/manifest/io.github.test%2Fdoes-not-exist?category=model",
        ))
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
    assert_eq!(body["catalog_version"], SEED_VERSION);
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
    //
    // Under v2 `current_version` is derived per-row from the catalog
    // ITEM's `version` field (1.0.0 for every seeded entry), not from
    // the catalog-wide `hub_version` (2.0.0). See `IndexItem.version`
    // + the per-entry stamping in `/hub/installed`'s handler.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let entity_id = Uuid::new_v4();
    // v2: hub_id is reverse-DNS, not slug. The §13.6 migration converts
    // pre-§12 slug rows; new test inserts use the v2 form directly.
    sqlx::query(
        "INSERT INTO hub_entities (id, entity_type, entity_id, hub_id, hub_category, hub_version)
         VALUES ($1, 'assistant', $2, 'io.github.phibya/code-reviewer', 'assistant', '0.0.0-test')",
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
    assert_eq!(items[0]["hub_id"], "io.github.phibya/code-reviewer");
    assert_eq!(items[0]["installed_version"], "0.0.0-test");
    // Per-entry version stamp: code-reviewer ships at 1.0.0 in the seed
    // (NOT the catalog-wide hub_version 2.0.0).
    assert_eq!(items[0]["current_version"], "1.0.0");
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
         VALUES ($1, 'mcp_server', $2, 'github', 'mcp_server')",
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

// v1's `/hub/releases` + `/hub/activate` tests have been removed:
// the endpoints are gone. The catalog is now refreshed in place from
// Pages by `/hub/refresh` (the read/manage perm split survives —
// covered by `catalog_read_cannot_refresh` above). The full
// publisher-switches-catalog flow is exercised hermetically in
// `catalog_hermetic.rs` via the mock Pages server's
// `MockHub::switch_to`.
