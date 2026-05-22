//! Tier 3 — HTTP integration tests for the environments admin surface:
//! the enriched `GET /api/code-sandbox/environments` fields and the new
//! `DELETE /api/code-sandbox/environments/{flavor}` evict endpoint.
//!
//! Permission model: GET requires `code_sandbox::environments::read`,
//! DELETE requires `code_sandbox::environments::manage`.

use serde_json::Value;

use crate::common::{test_helpers, TestServer};

fn env_url(server: &TestServer, suffix: &str) -> String {
    format!("{}/api/code-sandbox/environments{}", server.base_url, suffix)
}

async fn user_with_manage(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "env_manage",
        &["code_sandbox::environments::manage", "code_sandbox::environments::read"],
    )
    .await
    .token
}

async fn user_with_read_only(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "env_read",
        &["code_sandbox::environments::read"],
    )
    .await
    .token
}

// =====================================================================
// GET /environments — enriched fields
// =====================================================================

#[tokio::test]
async fn list_environments_exposes_cached_size_and_mounted_fields() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;

    let resp = reqwest::Client::new()
        .get(env_url(&server, ""))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status().as_u16(), 200);

    let body: Value = resp.json().await.expect("parse");
    let available = body["available"].as_array().expect("available array");
    assert!(!available.is_empty(), "expected at least one flavor");
    for env in available {
        // `cached_size_bytes` is present (null when not cached), `mounted` is a bool.
        assert!(
            env.get("cached_size_bytes").is_some(),
            "missing cached_size_bytes: {env}"
        );
        assert!(env.get("mounted").and_then(|v| v.as_bool()).is_some(), "missing mounted: {env}");
        // Not cached on a fresh server → size is null and mounted is false.
        if env["cached"].as_bool() == Some(false) {
            assert!(env["cached_size_bytes"].is_null(), "uncached → null size: {env}");
            assert_eq!(env["mounted"].as_bool(), Some(false), "uncached → not mounted: {env}");
        }
    }
}

// =====================================================================
// DELETE /environments/{flavor} — evict
// =====================================================================

#[tokio::test]
async fn evict_requires_manage_permission() {
    let server = TestServer::start().await;
    let read_token = user_with_read_only(&server).await;

    let resp = reqwest::Client::new()
        .delete(env_url(&server, "/minimal"))
        .header("Authorization", format!("Bearer {read_token}"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "read-only user must not be able to evict"
    );
}

#[tokio::test]
async fn evict_requires_authorization() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .delete(env_url(&server, "/minimal"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status().as_u16(), 401, "unauthenticated evict must be 401");
}

#[tokio::test]
async fn evict_unknown_flavor_returns_404() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let resp = reqwest::Client::new()
        .delete(env_url(&server, "/bogus-flavor"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status().as_u16(), 404, "unknown flavor must be 404");
}

#[tokio::test]
async fn evict_uncached_flavor_is_idempotent_noop() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    // Nothing is cached on a fresh server → 200 no-op, returns the refreshed
    // list with the flavor still not cached.
    let resp = reqwest::Client::new()
        .delete(env_url(&server, "/minimal"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);

    let body: Value = resp.json().await.expect("parse");
    let minimal = body["available"]
        .as_array()
        .expect("available")
        .iter()
        .find(|e| e["flavor"] == "minimal")
        .expect("minimal present");
    assert_eq!(minimal["cached"].as_bool(), Some(false));
}

// =====================================================================
// Real evict (needs a cached squashfs staged by the mirror fixture)
// =====================================================================

use crate::code_sandbox::mirror_fixture;

#[tokio::test]
async fn evict_removes_cached_squashfs_end_to_end() {
    let Some(fixture) = mirror_fixture::setup("minimal").await else {
        return; // skip: no bwrap / no dev squashfs
    };
    let token = test_helpers::create_user_with_permissions(
        &fixture.server,
        "env_evict_e2e",
        &["code_sandbox::environments::manage", "code_sandbox::environments::read"],
    )
    .await
    .token;
    let base = &fixture.server.base_url;

    // Prefetch minimal so it lands in the cache.
    let post = reqwest::Client::new()
        .post(format!("{base}/api/code-sandbox/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("prefetch");
    assert_eq!(post.status().as_u16(), 200);
    // Drain the SSE stream to completion so the install finishes.
    let _ = reqwest::Client::new()
        .get(format!("{base}/api/code-sandbox/prefetch/minimal/events"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("sse")
        .bytes()
        .await;

    // Now cached, with a real on-disk size.
    let listed: Value = reqwest::Client::new()
        .get(format!("{base}/api/code-sandbox/environments"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("parse");
    let minimal = listed["available"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["flavor"] == "minimal")
        .unwrap();
    assert_eq!(minimal["cached"].as_bool(), Some(true), "minimal should be cached after prefetch");
    assert!(
        minimal["cached_size_bytes"].as_u64().is_some_and(|n| n > 0),
        "cached_size_bytes should be > 0: {minimal}"
    );

    // Evict → 200, flavor flips to not-cached.
    let evicted: Value = reqwest::Client::new()
        .delete(format!("{base}/api/code-sandbox/environments/minimal"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("evict")
        .json()
        .await
        .expect("parse");
    let minimal_after = evicted["available"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["flavor"] == "minimal")
        .unwrap();
    assert_eq!(minimal_after["cached"].as_bool(), Some(false), "should be uncached after evict");
    assert!(minimal_after["cached_size_bytes"].is_null(), "size should be null after evict");
}
