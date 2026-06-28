//! Tier 2 — web_search admin settings: CRUD, permission gating, secret
//! round-trip (the API key is stored but never returned).

use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

fn admin_perms() -> &'static [&'static str] {
    &["web_search::admin::read", "web_search::admin::manage"]
}

/// A settings update must publish a `WebSearchSettings`/`update` sync frame
/// to holders of `web_search::admin::read` (handlers.rs:333-339), and must
/// NOT reach a user lacking that read perm (the audience is
/// `Audience::perm::<WebSearchAdminRead>()`). The wire id is `Uuid::nil`
/// because the settings row is a singleton.
#[tokio::test]
async fn test_web_search_settings_update_emits_sync_to_admins_only() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_sync_admin", admin_perms()).await;
    // Plain user: holds web_search::use (Users group) but no admin::read,
    // so it is outside the sync audience — a negative control.
    let plain =
        create_user_with_permissions(&server, "ws_sync_plain", &["web_search::use"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "max_results": 7 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let frame = admin_probe
        .expect_event("web_search_settings", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string());

    // The non-admin user is not in the audience and observes nothing.
    plain_probe.expect_silence(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn test_admin_can_get_and_update_settings() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    // GET returns the seeded singleton.
    let res = client
        .get(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["provider_chain"].is_array());

    // PUT updates a scalar + the chain.
    let res = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "max_results": 8, "provider_chain": ["brave", "searxng"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["max_results"], 8);
    assert_eq!(row["provider_chain"][0], "brave");
}

#[tokio::test]
async fn test_non_admin_cannot_update_settings() {
    let server = TestServer::start().await;
    // Plain user: has web_search::use (via Users group) but no admin perms.
    let user = create_user_with_permissions(&server, "ws_plain", &["web_search::use"]).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_provider_api_key_is_stored_but_never_returned() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_key_admin", admin_perms()).await;
    let client = reqwest::Client::new();
    const SECRET: &str = "BSA-super-secret-token-123";

    // Set the Brave key.
    let res = client
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": SECRET }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(
        !body.contains(SECRET),
        "PUT provider response must not echo the API key"
    );

    // GET the catalog: key marked set, value never present.
    let res = client
        .get(server.api_url("/web-search/providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(!body.contains(SECRET), "GET providers must not leak the key");
    let parsed: Value = serde_json::from_str(&body).unwrap();
    let brave = parsed["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == "brave")
        .expect("brave in catalog");
    assert_eq!(brave["api_key_set"], true);
    assert_eq!(brave["configured"], true);
}

#[tokio::test]
async fn test_unknown_provider_in_chain_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_chain_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "provider_chain": ["searxng", "nope"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_out_of_range_caps_are_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_cap_admin", admin_perms()).await;
    let client = reqwest::Client::new();
    // (field, out-of-range value) — below floor + above ceiling for each cap.
    let cases: &[(&str, Value)] = &[
        ("max_results", json!(0)),
        ("max_results", json!(999)),
        ("fetch_max_bytes", json!(1)),
        ("fetch_max_bytes", json!(200_000_000)),
        ("fetch_max_chars", json!(999)),
        ("fetch_max_chars", json!(600_000)),
        ("request_timeout_secs", json!(0)),
        ("request_timeout_secs", json!(100_000)),
    ];
    for (field, value) in cases {
        let mut map = serde_json::Map::new();
        map.insert((*field).to_string(), value.clone());
        let res = client
            .put(server.api_url("/web-search/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&Value::Object(map))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "{field}={value} must be rejected");
    }
}

#[tokio::test]
async fn test_unknown_provider_upsert_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_prov_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/providers/bogus"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "x": "y" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_empty_provider_chain_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_empty_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "provider_chain": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_malformed_searxng_base_url_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_url_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "base_url": "not a url" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

/// Fetch the provider catalog and return the entry for `provider`.
async fn get_provider_entry(
    server: &TestServer,
    token: &str,
    provider: &str,
) -> Value {
    let res = reqwest::Client::new()
        .get(server.api_url("/web-search/providers"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == provider)
        .cloned()
        .expect("provider in catalog")
}

#[tokio::test]
async fn test_api_key_clear_roundtrip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_clear_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    // Set the key → configured + api_key_set.
    let r = client
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "BSA-to-be-cleared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let brave = get_provider_entry(&server, &admin.token, "brave").await;
    assert_eq!(brave["api_key_set"], true);
    assert_eq!(brave["configured"], true);

    // Clear it (empty string) → key removed; brave (needs a key) goes unconfigured.
    let r = client
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let brave = get_provider_entry(&server, &admin.token, "brave").await;
    assert_eq!(brave["api_key_set"], false, "key must be cleared");
    assert_eq!(brave["configured"], false);
}

#[tokio::test]
async fn test_provider_chain_reorder_persists() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_order_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    for order in [["brave", "searxng"], ["searxng", "brave"]] {
        let r = client
            .put(server.api_url("/web-search/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({ "provider_chain": order }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let got: Value = client
            .get(server.api_url("/web-search/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(got["provider_chain"][0], order[0]);
        assert_eq!(got["provider_chain"][1], order[1]);
    }
}
