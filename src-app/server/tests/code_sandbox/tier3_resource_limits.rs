//! Tier 3 — HTTP integration tests for the resource-limits admin surface
//! (Plan 1 §6). GET returns the migration defaults; PUT updates + persists;
//! PUT without permission → 403; validation rejects out-of-range values
//! with 422.

use serde_json::{json, Value};

use crate::common::{test_helpers, TestServer};

fn url(server: &TestServer) -> String {
    format!("{}/api/code-sandbox/resource-limits", server.base_url)
}

async fn user_with_manage(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "limits_manage",
        &[
            "code_sandbox::resource_limits::read",
            "code_sandbox::resource_limits::manage",
        ],
    )
    .await
    .token
}

async fn user_with_read_only(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "limits_read",
        &["code_sandbox::resource_limits::read"],
    )
    .await
    .token
}

async fn user_without_any_limits_perm(server: &TestServer) -> String {
    // Pick an unrelated permission so the user is properly created.
    test_helpers::create_user_with_permissions(
        server,
        "limits_nope",
        &["code_sandbox::environments::read"],
    )
    .await
    .token
}

// =====================================================================
// GET — default-loaded singleton row
// =====================================================================

#[tokio::test]
async fn get_returns_migration_defaults_on_fresh_server() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;

    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);

    let body: Value = resp.json().await.expect("parse");
    // Every documented knob is exposed.
    for k in [
        "memory_max_bytes",
        "memory_swap_max_bytes",
        "pids_max",
        "cpu_max",
        "address_space_bytes",
        "fsize_bytes",
        "nproc_max",
        "nofile_max",
        "cpu_secs_max",
        "timeout_secs",
        "vm_idle_evict_secs",
        "created_at",
        "updated_at",
    ] {
        assert!(body.get(k).is_some(), "missing field {k}: {body}");
    }
    // Defaults match migration 41.
    assert_eq!(body["memory_max_bytes"].as_i64(), Some(512 * 1024 * 1024));
    assert_eq!(body["pids_max"].as_i64(), Some(256));
    assert_eq!(body["cpu_max"].as_str(), Some("100000 100000"));
    assert_eq!(body["timeout_secs"].as_i64(), Some(620));
}

#[tokio::test]
async fn get_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new().get(url(&server)).send().await.expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn get_requires_the_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_any_limits_perm(&server).await;
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

// =====================================================================
// PUT — update + persist + invalidate cache
// =====================================================================

#[tokio::test]
async fn put_updates_individual_fields_and_persists() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "memory_max_bytes": 256 * 1024 * 1024_i64, // halve from default 512 MiB
            "pids_max": 128,
            "timeout_secs": 300,
        }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);

    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["memory_max_bytes"].as_i64(), Some(256 * 1024 * 1024));
    assert_eq!(body["pids_max"].as_i64(), Some(128));
    assert_eq!(body["timeout_secs"].as_i64(), Some(300));
    // Untouched fields preserved (PATCH semantics).
    assert_eq!(body["cpu_max"].as_str(), Some("100000 100000"));

    // A subsequent GET sees the same values (persisted, not just cached).
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["memory_max_bytes"].as_i64(), Some(256 * 1024 * 1024));
    assert_eq!(body["pids_max"].as_i64(), Some(128));
    assert_eq!(body["timeout_secs"].as_i64(), Some(300));
}

#[tokio::test]
async fn put_requires_the_manage_permission() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;

    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "pids_max": 200 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn put_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .put(url(&server))
        .json(&json!({ "pids_max": 200 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

// =====================================================================
// PUT — value-range validation
// =====================================================================

#[tokio::test]
async fn put_rejects_out_of_range_memory() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    // Below the 16 MiB lower bound.
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "memory_max_bytes": 1_000 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 422, "got {:?}", resp.text().await);
}

#[tokio::test]
async fn put_rejects_pids_max_out_of_range() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    // 0 is below the 8-PID floor.
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "pids_max": 0 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn put_rejects_malformed_cpu_max() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    for bad in ["abc 100000", "100000", "100000 0", ""] {
        let resp = reqwest::Client::new()
            .put(url(&server))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "cpu_max": bad }))
            .send()
            .await
            .expect("request");
        assert_eq!(
            resp.status().as_u16(),
            422,
            "expected cpu_max={bad:?} to fail; got {:?}",
            resp.text().await
        );
    }
}

#[tokio::test]
async fn put_rejects_cpu_starvation() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    // 1 µs in 100 ms period = 0.001% of a CPU — would deadlock every job.
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "cpu_max": "1 100000" }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 422);
}
