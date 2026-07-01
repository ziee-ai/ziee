//! Tier 2/3 — per-user web_search provider keys: the masked read surface, the
//! set/clear roundtrip + validation, permission gating (403), the sync emit, and
//! the CORE behavioral proof that the caller's OWN key is resolved before the
//! deployment key at the search layer (a token-capturing mock Brave records
//! which key actually reached the upstream).

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::{jsonrpc, start_capturing_brave};
use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

fn admin_perms() -> &'static [&'static str] {
    &[
        "web_search::admin::read",
        "web_search::admin::manage",
        "web_search::use",
    ]
}

/// Set the deployment (shared) Brave key as an admin.
async fn set_deployment_brave_key(server: &TestServer, admin_token: &str, key: &str) {
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "api_key": key }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

/// Enable web search with the brave-only chain.
async fn enable_brave_chain(server: &TestServer, admin_token: &str) {
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "enabled": true, "provider_chain": ["brave"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn test_user_key_get_put_delete_roundtrip_masked() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "wsuk_admin", admin_perms()).await;
    let user = create_user_with_permissions(&server, "wsuk_user", &["web_search::use"]).await;
    let client = reqwest::Client::new();

    set_deployment_brave_key(&server, &admin.token, "DEPLOY-SHARED-KEY").await;

    // GET: catalog lists brave, no user key yet, but the shared key is flagged.
    let res = client
        .get(server.api_url("/web-search/user-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let brave = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["provider"] == "brave")
        .expect("brave in user catalog");
    assert_eq!(brave["system_key_set"], json!(true));
    assert!(brave["user_key"].is_null(), "no user key yet");
    // The deployment key value must NEVER appear anywhere in the response.
    assert!(
        !serde_json::to_string(&body)
            .unwrap()
            .contains("DEPLOY-SHARED-KEY"),
        "deployment key leaked into user catalog: {body}"
    );

    // PUT: set the user's own key.
    let res = client
        .put(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "api_key": "USER-OWN-KEY-123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let brave = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["provider"] == "brave")
        .unwrap();
    // Masked only — the raw key must never be echoed.
    assert_eq!(brave["user_key"], json!("USER***"));
    assert!(
        !serde_json::to_string(&body)
            .unwrap()
            .contains("USER-OWN-KEY-123"),
        "raw user key echoed back: {body}"
    );

    // DELETE: clears the user's key, falls back to the shared flag.
    let res = client
        .delete(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let res = client
        .get(server.api_url("/web-search/user-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let brave = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["provider"] == "brave")
        .unwrap();
    assert!(brave["user_key"].is_null(), "user key not cleared: {body}");
}

#[tokio::test]
async fn test_user_key_validation_and_unknown_provider() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "wsuk_val", &["web_search::use"]).await;
    let client = reqwest::Client::new();

    let put = |provider: &str, key: String| {
        client
            .put(server.api_url(&format!("/web-search/user-keys/{provider}")))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({ "api_key": key }))
            .send()
    };

    // empty → 400
    assert_eq!(put("brave", "   ".into()).await.unwrap().status(), 400);
    // too long → 400
    assert_eq!(put("brave", "x".repeat(501)).await.unwrap().status(), 400);
    // control chars → 400
    assert_eq!(
        put("brave", "abc\u{0007}def".into())
            .await
            .unwrap()
            .status(),
        400
    );
    // unknown provider → 400
    assert_eq!(put("nope", "valid-key".into()).await.unwrap().status(), 400);
    // a keyless provider (searxng) rejects a user key → 400
    assert_eq!(
        put("searxng", "valid-key".into()).await.unwrap().status(),
        400
    );
}

#[tokio::test]
async fn test_user_key_endpoints_require_use_permission() {
    let server = TestServer::start().await;
    // A user stripped of all groups → no web_search::use (the default Users
    // group grants it, so an empty explicit perm list is NOT enough).
    let outsider = create_user_with_no_permissions(&server, "wsuk_out").await;
    let client = reqwest::Client::new();

    let get = client
        .get(server.api_url("/web-search/user-keys"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 403, "GET must require web_search::use");

    let put = client
        .put(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .json(&json!({ "api_key": "k" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 403, "PUT must require web_search::use");

    let del = client
        .delete(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 403, "DELETE must require web_search::use");
}

#[tokio::test]
async fn test_user_key_resolves_before_deployment_key() {
    // The behavioral proof: a token-capturing mock Brave records which key
    // reached the upstream. User A has their own key; user B has none. A's own
    // key must win; B falls back to the deployment key; the two are isolated.
    let (brave_endpoint, seen) = start_capturing_brave().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_BRAVE_ENDPOINT".to_string(), brave_endpoint)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "wsuk_res_admin", admin_perms()).await;
    let user_a = create_user_with_permissions(&server, "wsuk_res_a", &["web_search::use"]).await;
    let user_b = create_user_with_permissions(&server, "wsuk_res_b", &["web_search::use"]).await;
    let client = reqwest::Client::new();

    set_deployment_brave_key(&server, &admin.token, "DEPLOY-KEY").await;
    enable_brave_chain(&server, &admin.token).await;

    // User A sets their own key.
    let r = client
        .put(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .json(&json!({ "api_key": "USER-A-KEY" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // (a) User A searches → their own key reaches the upstream.
    let res = jsonrpc(
        &server,
        &user_a.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "q" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);

    // (b) User B searches (no own key) → the deployment key reaches the upstream.
    let res = jsonrpc(
        &server,
        &user_b.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "q" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);

    let tokens = seen.lock().unwrap().clone();
    assert_eq!(
        tokens.len(),
        2,
        "two searches → two upstream calls: {tokens:?}"
    );
    assert_eq!(tokens[0], "USER-A-KEY", "user A's own key must win");
    assert_eq!(
        tokens[1], "DEPLOY-KEY",
        "user B must fall back to the deployment key"
    );
}

#[tokio::test]
async fn test_no_key_configured_errors_without_upstream_call() {
    // (d) Neither a user key nor a deployment key → typed unconfigured error,
    // and NO upstream call is made.
    let (brave_endpoint, seen) = start_capturing_brave().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_BRAVE_ENDPOINT".to_string(), brave_endpoint)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "wsuk_none_admin", admin_perms()).await;
    let user = create_user_with_permissions(&server, "wsuk_none_user", &["web_search::use"]).await;
    enable_brave_chain(&server, &admin.token).await;

    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "q" } }),
    )
    .send()
    .await
    .unwrap();
    // JSON-RPC returns 200 with an error object for a tool error.
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object() || body["result"]["isError"] == json!(true),
        "expected an error: {body}"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        0,
        "no upstream call when unconfigured"
    );
}

#[tokio::test]
async fn test_user_key_save_and_delete_emit_owner_scoped_sync() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "wsuk_sync_a", &["web_search::use"]).await;
    // A second user is the negative control: an owner-scoped emit must not reach them.
    let other = create_user_with_permissions(&server, "wsuk_sync_b", &["web_search::use"]).await;
    let client = reqwest::Client::new();

    let mut probe = SyncProbe::open(&server, &user.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let r = client
        .put(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "api_key": "USER-KEY" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let frame = probe
        .expect_event("web_search_user_key", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string());
    other_probe.expect_silence(Duration::from_secs(1)).await;

    let r = client
        .delete(server.api_url("/web-search/user-keys/brave"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    probe
        .expect_event("web_search_user_key", "delete", Duration::from_secs(5))
        .await;
}
