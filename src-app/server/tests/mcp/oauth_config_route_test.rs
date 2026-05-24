//! HTTP route tests for the per-server OAuth config endpoints (Phase 4):
//!   GET/PUT/DELETE `/api/mcp/servers/{id}/oauth`.
//!
//! Verifies set/get/delete round-trip, that the client secret is never
//! returned, validation, and ownership scoping (404 on someone else's server).

use crate::common::{test_helpers, TestServer};
use serde_json::json;
use uuid::Uuid;

/// Create an HTTP MCP server owned by `user`, returning its id.
async fn create_http_server(server: &TestServer, token: &str) -> Uuid {
    let payload = json!({
        "name": "oauth_server",
        "display_name": "OAuth Server",
        "enabled": true,
        "transport_type": "http",
        "url": "https://example.test/mcp",
        "timeout_seconds": 30
    });
    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .expect("create server");
    assert_eq!(resp.status(), 201, "server create should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn oauth_config_set_get_delete_roundtrip() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::read", "mcp_servers::edit"],
    )
    .await;
    let id = create_http_server(&server, &user.token).await;
    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/mcp/servers/{id}/oauth"));

    // Initially unset → null.
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_null());

    // Set the config.
    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "client_id": "mcp-client",
            "client_secret": "super-secret",
            "scopes": "mcp read"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "set oauth config should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["client_id"], "mcp-client");
    assert_eq!(body["has_client_secret"], true);
    assert_eq!(body["scopes"], "mcp read");
    assert!(
        body.get("client_secret").is_none(),
        "response MUST NOT include the client secret"
    );

    // Get it back — still no secret.
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["client_id"], "mcp-client");
    assert!(body.get("client_secret").is_none());

    // Delete → 204, then GET → null.
    let resp = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(resp.json::<serde_json::Value>().await.unwrap().is_null());
}

#[tokio::test]
async fn oauth_config_set_rejects_empty_client_id() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::read", "mcp_servers::edit"],
    )
    .await;
    let id = create_http_server(&server, &user.token).await;

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/mcp/servers/{id}/oauth")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "client_id": "", "client_secret": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "empty client_id must be rejected");
}

#[tokio::test]
async fn oauth_config_on_unknown_server_returns_404() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::read", "mcp_servers::edit"],
    )
    .await;

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/mcp/servers/{}/oauth", Uuid::new_v4())))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "client_id": "c", "client_secret": "s" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "OAuth config on a non-owned/unknown server must 404");
}

#[tokio::test]
async fn oauth_config_survives_a_server_update() {
    // Backend side of the UI's "leave the secret blank to keep it": editing the
    // server row (without re-PUTing OAuth) must not disturb the stored config.
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::read", "mcp_servers::edit"],
    )
    .await;
    let id = create_http_server(&server, &user.token).await;
    let client = reqwest::Client::new();
    let oauth_url = server.api_url(&format!("/mcp/servers/{id}/oauth"));

    // Store an OAuth config (with a secret).
    let resp = client
        .put(&oauth_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "client_id": "mcp-client", "client_secret": "super-secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Update the server row (display name only) — OAuth lives in its own table
    // and must be untouched.
    let resp = client
        .put(server.api_url(&format!("/mcp/servers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "display_name": "Renamed Server" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "server update should succeed");

    // The OAuth config — and its stored secret — must still be there.
    let resp = client
        .get(&oauth_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["client_id"], "mcp-client", "client id must survive the update");
    assert_eq!(
        body["has_client_secret"], true,
        "the stored secret must survive a server-row update (leave-blank-keeps)"
    );
}
