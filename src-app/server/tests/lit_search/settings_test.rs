//! Tier 2 — lit_search admin settings + connectors: CRUD, permission gating,
//! secret round-trip (key stored, never returned), validation, and the
//! `lit_search_settings` sync publish.

use std::time::Duration;

use serde_json::{Value, json};

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

fn admin_perms() -> &'static [&'static str] {
    &["lit_search::admin::read", "lit_search::admin::manage"]
}

/// Fetch the connector catalog and return the entry for `connector`.
async fn get_connector(server: &TestServer, token: &str, connector: &str) -> Value {
    let res = reqwest::Client::new()
        .get(server.api_url("/lit-search/connectors"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    body["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == connector)
        .cloned()
        .unwrap_or_else(|| panic!("{connector} in catalog"))
}

#[tokio::test]
async fn test_admin_can_get_and_update_settings() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    let res = client
        .get(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["enabled_connectors"].is_array());

    let res = client
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "max_results": 30, "enabled_connectors": ["europepmc", "arxiv"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["max_results"], 30);
    assert_eq!(row["enabled_connectors"][0], "europepmc");
}

#[tokio::test]
async fn test_non_admin_cannot_update_settings() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_plain", &["lit_search::use"]).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_connector_api_key_is_stored_but_never_returned() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_key_admin", admin_perms()).await;
    let client = reqwest::Client::new();
    const SECRET: &str = "CORE-super-secret-token-123";

    // CORE's key is required; set it.
    let res = client
        .put(server.api_url("/lit-search/connectors/core"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": SECRET }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(!body.contains(SECRET), "PUT connector response must not echo the key");

    let res = client
        .get(server.api_url("/lit-search/connectors"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert!(!body.contains(SECRET), "GET connectors must not leak the key");
    let core = get_connector(&server, &admin.token, "core").await;
    assert_eq!(core["api_key_set"], true);
}

#[tokio::test]
async fn test_api_key_clear_roundtrip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_clear_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    client
        .put(server.api_url("/lit-search/connectors/core"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "CORE-to-be-cleared" }))
        .send()
        .await
        .unwrap();
    let core = get_connector(&server, &admin.token, "core").await;
    assert_eq!(core["api_key_set"], true);

    // Empty string clears it (tri-state); CORE (needs a key) goes unconfigured.
    let r = client
        .put(server.api_url("/lit-search/connectors/core"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let core = get_connector(&server, &admin.token, "core").await;
    assert_eq!(core["api_key_set"], false, "key must be cleared");
    assert_eq!(core["configured"], false, "CORE without a key is unconfigured");
}

#[tokio::test]
async fn test_connector_config_persists() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_cfg_admin", admin_perms()).await;
    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/connectors/crossref"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "mailto": "researcher@example.org" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let crossref = get_connector(&server, &admin.token, "crossref").await;
    assert_eq!(crossref["configured"], true);
}

#[tokio::test]
async fn test_unknown_connector_in_enabled_list_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_unk_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled_connectors": ["europepmc", "nope"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_empty_enabled_connectors_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_empty_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled_connectors": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_out_of_range_caps_are_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_cap_admin", admin_perms()).await;
    let client = reqwest::Client::new();
    let cases: &[(&str, Value)] = &[
        ("max_results", json!(0)),
        ("max_results", json!(201)),
        ("per_source_limit", json!(0)),
        ("per_source_limit", json!(501)),
        ("request_timeout_secs", json!(0)),
        ("request_timeout_secs", json!(121)),
    ];
    for (field, value) in cases {
        let mut map = serde_json::Map::new();
        map.insert((*field).to_string(), value.clone());
        let res = client
            .put(server.api_url("/lit-search/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&Value::Object(map))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "{field}={value} must be rejected");
    }
}

#[tokio::test]
async fn test_unknown_connector_upsert_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_bogus_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/lit-search/connectors/bogus"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "x": "y" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_settings_update_publishes_sync_event() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_sync_admin", admin_perms()).await;
    // Subscribe BEFORE the mutation (admin holds lit_search::admin::read, the
    // audience perm for LitSearchSettings).
    let mut probe = SyncProbe::open(&server, &admin.token).await;

    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "max_results": 12 }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // snake_case serialization of SyncEntity::LitSearchSettings / SyncAction::Update.
    probe
        .expect_event("lit_search_settings", "update", Duration::from_secs(5))
        .await;
}

/// The SECOND lit_search emit point: a connector config update (PUT
/// /lit-search/connectors/{connector}) also publishes LitSearchSettings/update
/// to admins (handlers.rs update_connector), distinct from the settings PUT
/// covered above. A use-only (non-admin) user must NOT observe it.
#[tokio::test]
async fn test_connector_update_publishes_sync_event() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_conn_admin", admin_perms()).await;
    let plain =
        create_user_with_permissions(&server, "ls_conn_plain", &["lit_search::use"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/connectors/crossref"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "cr-secret-123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "connector update should 200");

    admin_probe
        .expect_event("lit_search_settings", "update", Duration::from_secs(5))
        .await;
    plain_probe.expect_silence(Duration::from_secs(2)).await;
}
