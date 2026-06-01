//! Integration tests for the MCP "test connection" endpoints
//! (`POST /mcp/servers/test-connection` + `POST /mcp/system-servers/test-connection`).
//!
//! These probe a *candidate* server config without persisting it. Success and
//! failure are both returned as HTTP 200 with a `{ success, message, tool_count }`
//! body — only `success` is authoritative. The in-process [`MockSamplingServer`]
//! answers `initialize` + `tools/list`, so it doubles as a reachable HTTP MCP
//! server for the happy path.

use super::mock_sampling_server::MockSamplingServer;
use crate::common::test_helpers;
use serde_json::json;

// ============================================================================
// User endpoint
// ============================================================================

#[tokio::test]
async fn test_connection_http_success() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;
    let mock = MockSamplingServer::start().await;

    let payload = json!({
        "transport_type": "http",
        "url": mock.url(),
        "timeout_seconds": 10
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 200, "got body: {body}");
    assert_eq!(body["success"], true, "expected success, got: {body}");
    // The mock advertises exactly one tool.
    assert_eq!(body["tool_count"], 1);
    assert!(body["message"].as_str().unwrap().contains("Connected"));
}

#[tokio::test]
async fn test_connection_http_unreachable() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({
        "transport_type": "http",
        // Port 1 is privileged + unbound → connection refused, fails fast.
        "url": "http://127.0.0.1:1",
        "timeout_seconds": 3
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    // Connection failure is still a 200 — the result lives in the body.
    assert_eq!(status, 200, "got body: {body}");
    assert_eq!(body["success"], false, "expected failure, got: {body}");
    assert!(
        !body["message"].as_str().unwrap().is_empty(),
        "failure must carry a message"
    );
    assert!(body["tool_count"].is_null());
}

#[tokio::test]
async fn test_connection_stdio_failure() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({
        "transport_type": "stdio",
        // Outside the stdio allowlist → connect() fails before any spawn.
        "command": "definitely-not-a-real-binary",
        "args": [],
        "timeout_seconds": 5
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 200, "got body: {body}");
    assert_eq!(body["success"], false, "expected failure, got: {body}");
}

#[tokio::test]
async fn test_connection_validation_rejects_http_without_url() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({ "transport_type": "http" });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // Missing URL is a config error → 400, distinct from a connection failure.
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_connection_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    // Read-only user with NO default-group inheritance: can view servers but
    // not create/test configs. `create_user_with_permissions` would also add
    // the default "Users" group (which grants `mcp_servers::*`), masking the gate.
    let user = test_helpers::create_user_with_only_permissions(
        &server,
        "reader",
        &["mcp_servers::read"],
    )
    .await;

    let payload = json!({
        "transport_type": "http",
        "url": "http://127.0.0.1:9",
        "timeout_seconds": 2
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "read-only user must be forbidden");
}

#[tokio::test]
async fn test_connection_reuses_stored_oauth_for_existing_server() {
    // Create an HTTP user server pointing at the mock, attach an OAuth config,
    // then test by `id` with the SAME url and no inline oauth. This exercises
    // the stored-secret fallback in `resolve_oauth` (the mock ignores auth, so
    // the test still succeeds — the point is the path executes without error).
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::edit", "mcp_servers::read"],
    )
    .await;
    let mock = MockSamplingServer::start().await;
    let client = reqwest::Client::new();

    // Create the server.
    let created: serde_json::Value = client
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "oauth_probe_server",
            "display_name": "OAuth Probe",
            "transport_type": "http",
            "url": mock.url(),
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("create failed")
        .json()
        .await
        .expect("parse create");
    let id = created["id"].as_str().expect("server id").to_string();

    // Attach an OAuth config (write-only secret).
    let oauth_status = client
        .put(server.api_url(&format!("/mcp/servers/{id}/oauth")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "client_id": "cid", "client_secret": "shhh" }))
        .send()
        .await
        .expect("oauth set failed")
        .status();
    assert_eq!(oauth_status, 200);

    // Test by id, same url, no inline oauth → stored secret path.
    let body: serde_json::Value = client
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "transport_type": "http",
            "url": mock.url(),
            "id": id,
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("test-connection failed")
        .json()
        .await
        .expect("parse body");

    assert_eq!(body["success"], true, "got: {body}");
    assert_eq!(body["tool_count"], 1);
}

// ============================================================================
// System (admin) endpoint
// ============================================================================

#[tokio::test]
async fn test_system_connection_http_success() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let mock = MockSamplingServer::start().await;

    let payload = json!({
        "transport_type": "http",
        "url": mock.url(),
        "timeout_seconds": 10
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers/test-connection"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 200, "got body: {body}");
    assert_eq!(body["success"], true, "got: {body}");
    assert_eq!(body["tool_count"], 1);
}

#[tokio::test]
async fn test_system_connection_requires_admin_create_permission() {
    let server = crate::common::TestServer::start().await;
    // A user-level create permission must NOT unlock the system endpoint.
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({
        "transport_type": "http",
        "url": "http://127.0.0.1:9",
        "timeout_seconds": 2
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "non-admin must be forbidden");
}
