//! Tier 2/3 — per-user lit_search connector keys: the masked read surface, the
//! set/clear roundtrip + validation, permission gating (403), the sync emit, and
//! the CORE behavioral proof that the caller's OWN key is resolved before the
//! deployment key at the search layer (a token-capturing mock CORE records which
//! bearer actually reached the upstream).

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::{configure, jsonrpc, start_capturing_core};
use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

fn admin_perms() -> &'static [&'static str] {
    &[
        "lit_search::admin::read",
        "lit_search::admin::manage",
        "lit_search::use",
    ]
}

async fn set_deployment_core_key(server: &TestServer, admin_token: &str, key: &str) {
    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/connectors/core"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "api_key": key }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn test_user_key_get_put_delete_roundtrip_masked() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "lsuk_admin", admin_perms()).await;
    let user = create_user_with_permissions(&server, "lsuk_user", &["lit_search::use"]).await;
    let client = reqwest::Client::new();

    set_deployment_core_key(&server, &admin.token, "DEPLOY-SHARED-KEY").await;

    // GET: catalog lists the key-accepting connectors incl. core; core shows the
    // shared key flag but no user key yet. The deployment value never appears.
    let res = client
        .get(server.api_url("/lit-search/user-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let core = body["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["connector"] == "core")
        .expect("core in user catalog");
    assert_eq!(core["system_key_set"], json!(true));
    assert!(core["user_key"].is_null());
    assert!(
        !serde_json::to_string(&body)
            .unwrap()
            .contains("DEPLOY-SHARED-KEY"),
        "deployment key leaked: {body}"
    );

    // PUT: set the user's own key → masked-only echo.
    let res = client
        .put(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "api_key": "USER-OWN-KEY-123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let core = body["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["connector"] == "core")
        .unwrap();
    assert_eq!(core["user_key"], json!("USER***"));
    assert!(
        !serde_json::to_string(&body)
            .unwrap()
            .contains("USER-OWN-KEY-123"),
        "raw user key echoed: {body}"
    );

    // DELETE → cleared.
    let res = client
        .delete(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let res = client
        .get(server.api_url("/lit-search/user-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let core = body["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["connector"] == "core")
        .unwrap();
    assert!(core["user_key"].is_null());
}

#[tokio::test]
async fn test_user_key_validation_and_unknown_connector() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "lsuk_val", &["lit_search::use"]).await;
    let client = reqwest::Client::new();

    let put = |connector: &str, key: String| {
        client
            .put(server.api_url(&format!("/lit-search/user-keys/{connector}")))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({ "api_key": key }))
            .send()
    };

    assert_eq!(put("core", "  ".into()).await.unwrap().status(), 400);
    assert_eq!(put("core", "x".repeat(501)).await.unwrap().status(), 400);
    assert_eq!(
        put("core", "abc\u{0007}def".into()).await.unwrap().status(),
        400
    );
    assert_eq!(put("nope", "valid-key".into()).await.unwrap().status(), 400);
    // A keyless connector (europepmc has no key_field) rejects a user key → 400.
    assert_eq!(
        put("europepmc", "valid-key".into()).await.unwrap().status(),
        400
    );
}

#[tokio::test]
async fn test_user_key_endpoints_require_use_permission() {
    let server = TestServer::start().await;
    // A user stripped of all groups → no lit_search::use (the default Users
    // group grants it, so an empty explicit perm list is NOT enough).
    let outsider = create_user_with_no_permissions(&server, "lsuk_out").await;
    let client = reqwest::Client::new();

    let get = client
        .get(server.api_url("/lit-search/user-keys"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 403);

    let put = client
        .put(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .json(&json!({ "api_key": "k" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 403);

    let del = client
        .delete(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 403);
}

#[tokio::test]
async fn test_user_key_resolves_before_deployment_key() {
    let (core_endpoint, seen) = start_capturing_core().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_CORE_ENDPOINT".to_string(), core_endpoint),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "lsuk_res_admin", admin_perms()).await;
    let user_a = create_user_with_permissions(&server, "lsuk_res_a", &["lit_search::use"]).await;
    let user_b = create_user_with_permissions(&server, "lsuk_res_b", &["lit_search::use"]).await;
    let client = reqwest::Client::new();

    set_deployment_core_key(&server, &admin.token, "DEPLOY-KEY").await;
    // Only CORE enabled so the UNION fans out to exactly the one upstream we capture.
    configure(&server, &admin.token, &["core"]).await;

    // User A sets their own key.
    let r = client
        .put(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .json(&json!({ "api_key": "USER-A-KEY" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // (a) User A searches → their own key reaches the upstream.
    let r = jsonrpc(
        &server,
        &user_a.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "cancer" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(r.status(), 200);

    // (b) User B searches (no own key) → the deployment key reaches the upstream.
    let r = jsonrpc(
        &server,
        &user_b.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "cancer" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(r.status(), 200);

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
async fn test_user_key_save_and_delete_emit_owner_scoped_sync() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "lsuk_sync_a", &["lit_search::use"]).await;
    let other = create_user_with_permissions(&server, "lsuk_sync_b", &["lit_search::use"]).await;
    let client = reqwest::Client::new();

    let mut probe = SyncProbe::open(&server, &user.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let r = client
        .put(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "api_key": "USER-KEY" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let frame = probe
        .expect_event("lit_search_user_key", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string());
    other_probe.expect_silence(Duration::from_secs(1)).await;

    let r = client
        .delete(server.api_url("/lit-search/user-keys/core"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    probe
        .expect_event("lit_search_user_key", "delete", Duration::from_secs(5))
        .await;
}
