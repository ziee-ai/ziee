//! Integration tests for the admin-configurable run_js limits
//! (`js_tool_settings`). GET returns the migration defaults; PUT updates +
//! persists; permission gate (401/403); validation (422); the DB value is
//! honored at execution (cache invalidation flips a run_js outcome); and the
//! PUT emits the `js_tool_settings` sync entity. Mirrors code_sandbox's
//! `tier3_resource_limits` + `sync_emit_test`.

use std::time::Duration;

use serde_json::{Value, json};

use crate::common::stub_chat::{StubChat, register_stub_model};
use crate::common::sync_probe::SyncProbe;
use crate::common::{TestServer, test_helpers};

fn url(server: &TestServer) -> String {
    server.api_url("/js-tool/settings")
}

async fn admin_token(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "jsset_admin",
        &["js_tool::settings::read", "js_tool::settings::manage"],
    )
    .await
    .token
}

/// A plain run_js user: holds `js_tool::use` but NOT the settings perms.
async fn plain_user_token(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(server, "jsset_plain", &["js_tool::use"])
        .await
        .token
}

// ── GET ──────────────────────────────────────────────────────────────────────

// TEST-42: GET as admin returns the seeded default row.
#[tokio::test]
async fn get_returns_migration_defaults() {
    let server = TestServer::start().await;
    let token = admin_token(&server).await;
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);
    let body: Value = resp.json().await.expect("parse");
    for k in [
        "memory_bytes",
        "max_stack_bytes",
        "wall_secs",
        "approval_timeout_secs",
        "max_concurrent_runs",
        "max_concurrent_dispatch",
        "max_trace_entries",
        "created_at",
        "updated_at",
    ] {
        assert!(body.get(k).is_some(), "missing field {k}: {body}");
    }
    assert_eq!(body["memory_bytes"].as_i64(), Some(128 * 1024 * 1024));
    assert_eq!(body["max_stack_bytes"].as_i64(), Some(512 * 1024));
    assert_eq!(body["wall_secs"].as_i64(), Some(300));
    assert_eq!(body["approval_timeout_secs"].as_i64(), Some(300));
    assert_eq!(body["max_concurrent_runs"].as_i64(), Some(8));
    assert_eq!(body["max_concurrent_dispatch"].as_i64(), Some(6));
    assert_eq!(body["max_trace_entries"].as_i64(), Some(256));
}

// TEST-46: unauthenticated → 401.
#[tokio::test]
async fn get_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new().get(url(&server)).send().await.expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

// TEST-45 (read half): a plain run_js user (no settings::read) → 403 on GET.
#[tokio::test]
async fn get_requires_the_read_permission() {
    let server = TestServer::start().await;
    let token = plain_user_token(&server).await;
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

// ── PUT ──────────────────────────────────────────────────────────────────────

// TEST-43: PUT a partial patch → 200 + updated row; a subsequent GET persists it.
#[tokio::test]
async fn put_updates_fields_and_persists() {
    let server = TestServer::start().await;
    let token = admin_token(&server).await;
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "wall_secs": 120, "max_concurrent_runs": 16 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["wall_secs"].as_i64(), Some(120));
    assert_eq!(body["max_concurrent_runs"].as_i64(), Some(16));
    // Untouched fields preserved (PATCH).
    assert_eq!(body["memory_bytes"].as_i64(), Some(128 * 1024 * 1024));

    // A subsequent GET sees the persisted values.
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["wall_secs"].as_i64(), Some(120));
    assert_eq!(body["max_concurrent_runs"].as_i64(), Some(16));
}

// TEST-45 (manage half): a plain user → 403 on PUT.
#[tokio::test]
async fn put_requires_the_manage_permission() {
    let server = TestServer::start().await;
    let token = plain_user_token(&server).await;
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "wall_secs": 120 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

// TEST-44: absurd values are rejected with 422 (validation-rejects-absurd).
#[tokio::test]
async fn put_rejects_out_of_range_values() {
    let server = TestServer::start().await;
    let token = admin_token(&server).await;
    for (field, bad) in [
        ("memory_bytes", json!(1)),                    // < 16 MiB
        ("memory_bytes", json!(8 * 1024 * 1024 * 1024i64)), // > 4 GiB
        ("max_stack_bytes", json!(1024)),              // < 64 KiB
        ("wall_secs", json!(0)),                       // < 1
        ("wall_secs", json!(99999)),                   // > 3600
        ("max_concurrent_runs", json!(100000)),        // > 256
        ("max_concurrent_dispatch", json!(0)),         // < 1
        ("max_trace_entries", json!(0)),               // < 1
    ] {
        let resp = reqwest::Client::new()
            .put(url(&server))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ field: bad }))
            .send()
            .await
            .expect("request");
        assert_eq!(
            resp.status().as_u16(),
            422,
            "expected 422 for {field}={bad}; got {:?}",
            resp.text().await
        );
    }
    // The absurd PUTs did not mutate the row.
    let resp = reqwest::Client::new()
        .get(url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["memory_bytes"].as_i64(), Some(128 * 1024 * 1024));
    assert_eq!(body["wall_secs"].as_i64(), Some(300));
}

// ── db-value-honored-at-execution ────────────────────────────────────────────

// TEST-47: a low `memory_bytes` set via PUT is honored on the very next run_js
// (cache invalidation → JsCaps::from_settings → live evaluation). The 40 MiB
// alloc SUCCEEDS under the 128 MiB default but OOMs under a 16 MiB cap.
#[tokio::test]
async fn db_memory_cap_is_honored_at_execution() {
    let server = TestServer::start().await;
    let user = super::power_user(&server, "jsset_exec").await;
    let stub = StubChat::start().await;
    let model_id =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;

    // Positive control: under the 128 MiB default the 40 MiB alloc succeeds → 40.
    let (conv, branch) = super::create_conversation(&server, &user, &model_id).await;
    let text = super::send_collect(&server, &user, &conv, &branch, &model_id, "STUB_PLAN=run_js_bigalloc go").await;
    assert!(text.contains("40"), "default cap should let the 40 MiB alloc succeed: {text}");
    assert!(!text.contains("run_js error"), "no error expected at default cap: {text}");

    // Tighten the memory cap to 16 MiB via the admin PUT.
    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "memory_bytes": 16 * 1024 * 1024 }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200, "PUT: {:?}", resp.text().await);

    // Now the SAME script OOMs — proving the DB value + cache invalidation took
    // effect at execution without a restart.
    let (conv2, branch2) = super::create_conversation(&server, &user, &model_id).await;
    let text2 = super::send_collect(&server, &user, &conv2, &branch2, &model_id, "STUB_PLAN=run_js_bigalloc go").await;
    assert!(
        text2.contains("run_js error"),
        "the 16 MiB DB cap must be honored → the 40 MiB alloc errors: {text2}"
    );
}

// ── sync emit ────────────────────────────────────────────────────────────────

// TEST-48: a successful PUT emits `js_tool_settings`/`update` to the read
// audience; a user lacking the read perm stays silent.
#[tokio::test]
async fn put_emits_js_tool_settings_sync_event() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "jsset_sync_admin",
        &["js_tool::settings::read", "js_tool::settings::manage"],
    )
    .await;
    let bob = test_helpers::create_user_with_permissions(&server, "jsset_sync_bob", &[]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let resp = reqwest::Client::new()
        .put(url(&server))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "max_trace_entries": 512 }))
        .send()
        .await
        .expect("PUT failed");
    assert_eq!(resp.status().as_u16(), 200, "PUT: {:?}", resp.text().await);

    admin_probe
        .expect_event("js_tool_settings", "update", Duration::from_secs(5))
        .await;
    bob_probe.expect_silence(Duration::from_secs(1)).await;
}
