// MCP HTTP transport integration tests
// Tests for MCP server with HTTP transport (Streamable HTTP)
//
// These tests cover the full request stack: HTTP route → permission gate →
// repository → session manager → HttpMcpClient → upstream MCP server. The
// upstream server is `@modelcontextprotocol/server-everything` spawned via
// the `EverythingServer` fixture; tests that need an upstream server return
// early (skip) when `npx` is not on PATH.

use super::fixtures::everything_server::EverythingServer;
use crate::common::test_helpers::{self, TestUser};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Helper Functions
// ============================================================================

/// Spawn `server-everything` and register it in the test DB as a system MCP
/// server. Returns the upstream fixture (kept alive by the caller's scope)
/// and the DB row id. Returns `None` if the fixture can't start (e.g., no
/// node in CI) — the test should `return;` in that case.
async fn create_http_everything_server(
    server: &crate::common::TestServer,
    admin: &TestUser,
    test_name: &str,
) -> Option<(EverythingServer, Uuid)> {
    let everything = EverythingServer::try_start_or_skip(test_name).await?;

    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("test_http_everything_{}", &unique_id[..8]),
        "display_name": "Test HTTP Everything Server",
        "description": "Reference MCP server (HTTP/Streamable HTTP transport)",
        "enabled": true,
        "transport_type": "http",
        "url": everything.base_url(),
        "timeout_seconds": 30
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create server");

    assert_eq!(
        response.status(),
        201,
        "Should create HTTP server successfully"
    );

    let body: serde_json::Value = response.json().await.unwrap();
    let server_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    Some((everything, server_id))
}

// ============================================================================
// List Tools Tests
// ============================================================================

#[tokio::test]
async fn test_http_list_server_tools() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_server_tools").await
    else { return; };

    // List tools from the server
    let url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");

    assert_eq!(status, 200, "Should list tools (status {}, body: {})", status, body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    let tools = body["tools"].as_array().expect("Should have tools array");
    assert!(!tools.is_empty(), "server-everything exposes multiple tools");

    // server-everything exposes `echo`
    let has_echo = tools.iter().any(|t| t["name"].as_str() == Some("echo"));
    assert!(has_echo, "expected `echo` tool from server-everything");

    let first_tool = tools.first().unwrap();
    assert!(first_tool["name"].is_string(), "Tool should have name");
    assert!(first_tool["input_schema"].is_object(), "Tool should have input_schema");
}

#[tokio::test]
async fn test_http_list_tools_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_tools_permission_required").await
    else { return; };

    // User without permission tries to list tools
    let url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_list_tools_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Use random UUID for server_id
    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/tools", random_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server (more secure - doesn't reveal if server exists)");
}

// ============================================================================
// Call Tool Tests
// ============================================================================

#[tokio::test]
async fn test_http_call_echo_tool() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_call_echo_tool").await
    else { return; };

    // Call `echo` — simplest round-trip on server-everything.
    let url = server.api_url(&format!("/mcp/servers/{}/tools/echo/call", server_id));
    let payload = json!({
        "arguments": { "message": "http-route-canary" }
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");
    assert_eq!(status, 200, "echo call should succeed (body: {})", body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    assert!(body["content"].is_array(), "Should have content array");
    let combined = body["content"].to_string();
    assert!(combined.contains("http-route-canary"),
            "echo response must include our input; got: {}", combined);
}

#[tokio::test]
async fn test_http_call_tool_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_call_tool_permission_required").await
    else { return; };

    // User without permission tries to call tool
    let url = server.api_url(&format!("/mcp/servers/{}/tools/echo/call", server_id));
    let payload = json!({ "arguments": { "message": "denied" } });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_call_tool_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/echo/call", random_id));
    let payload = json!({ "arguments": { "message": "x" } });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server (more secure - doesn't reveal if server exists)");
}

// ============================================================================
// List Resources Tests
// ============================================================================

#[tokio::test]
async fn test_http_list_server_resources() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_server_resources").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/resources", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert_eq!(status, 200, "server-everything supports resources (body: {})", body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    assert!(body["resources"].is_array(), "Should have resources array");
}

#[tokio::test]
async fn test_http_list_resources_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_resources_permission_required").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/resources", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

// ============================================================================
// Disconnect Server Tests
// ============================================================================

#[tokio::test]
async fn test_http_disconnect_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_disconnect_server").await
    else { return; };

    // First connect by listing tools
    let list_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(list_response.status(), 200, "Should connect to HTTP server successfully");

    let disconnect_url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&disconnect_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should disconnect HTTP server successfully");
}

#[tokio::test]
async fn test_http_disconnect_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_disconnect_permission_required").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_disconnect_idempotent() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_disconnect_idempotent").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response1 = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response1.status(), 200, "First disconnect should succeed");

    let response2 = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response2.status(), 200, "Second disconnect should also succeed (idempotent)");
}

// ============================================================================
// Concurrent Tests
// ============================================================================

#[tokio::test]
async fn test_http_concurrent_tool_calls() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_concurrent_tool_calls").await
    else { return; };

    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/echo/call", server_id));

    let mut handles = vec![];
    for i in 0..3 {
        let client = client.clone();
        let url = url.clone();
        let token = admin.token.clone();
        let payload = json!({ "arguments": { "message": format!("concurrent-{}", i) } });

        let handle = tokio::spawn(async move {
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&payload)
                .send()
                .await
                .unwrap_or_else(|_| panic!("Request {} failed", i));

            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            (i, status, body_text)
        });

        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;

    for result in results {
        let (i, status, body) = result.expect("Task panicked");
        assert_eq!(status, 200, "Concurrent HTTP request {} failed (body: {})", i, body);
        assert!(body.contains(&format!("concurrent-{}", i)),
                "Concurrent request {} returned wrong content (id mixup?): {}", i, body);
    }
}

// ============================================================================
// Prompts Tests (new in feat/mcp-rewrite-v2)
// ============================================================================

#[tokio::test]
async fn test_http_list_server_prompts() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_server_prompts").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/prompts", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert_eq!(status, 200, "Should list prompts (body: {})", body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    let prompts = body["prompts"].as_array().expect("Should have prompts array");
    assert!(!prompts.is_empty(), "server-everything exposes prompts");
    let first = &prompts[0];
    assert!(first["name"].is_string(), "Prompt should have name");
}

#[tokio::test]
async fn test_http_list_prompts_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_list_prompts_permission_required").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/prompts", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_list_prompts_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/prompts", random_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server");
}

#[tokio::test]
async fn test_http_get_server_prompt() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_get_server_prompt").await
    else { return; };

    // First list prompts to find one we can render
    let list_url = server.api_url(&format!("/mcp/servers/{}/prompts", server_id));
    let list_body: serde_json::Value = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prompts = list_body["prompts"].as_array().expect("prompts array");
    let target = prompts
        .iter()
        .find(|p| p["arguments"].as_array().map(|a| a.is_empty()).unwrap_or(true))
        .or_else(|| prompts.first())
        .expect("at least one prompt available");
    let name = target["name"].as_str().expect("prompt name");

    // Build args object (empty if none required)
    let mut args = serde_json::Map::new();
    if let Some(arg_array) = target["arguments"].as_array() {
        for a in arg_array {
            if let Some(arg_name) = a["name"].as_str() {
                args.insert(arg_name.to_string(),
                            serde_json::Value::String("test-value".to_string()));
            }
        }
    }

    let url = server.api_url(&format!("/mcp/servers/{}/prompts/get", server_id));
    let payload = serde_json::json!({
        "name": name,
        "arguments": serde_json::Value::Object(args),
    });
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert_eq!(status, 200, "get_prompt should succeed (body: {})", body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("parse");
    // GetPromptResponse flattens PromptResult, so the body is {description, messages} directly.
    let messages = body["messages"].as_array()
        .unwrap_or_else(|| panic!("messages array missing or wrong type; full body: {}", body));
    assert!(!messages.is_empty(), "rendered prompt must yield messages");
}

#[tokio::test]
async fn test_http_get_prompt_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_get_prompt_permission_required").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/prompts/get", server_id));
    let payload = json!({ "name": "anything", "arguments": {} });
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("parse");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_get_prompt_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/prompts/get", random_id));
    let payload = json!({ "name": "anything", "arguments": {} });
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server");
}

// ============================================================================
// Ping Tests (new in feat/mcp-rewrite-v2)
// ============================================================================

#[tokio::test]
async fn test_http_ping_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_ping_server").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/ping", server_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert_eq!(status, 200, "ping should succeed (body: {})", body_text);
    let body: serde_json::Value = serde_json::from_str(&body_text).expect("parse");
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn test_http_ping_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let Some((_upstream, server_id)) =
        create_http_everything_server(&server, &admin, "test_http_ping_permission_required").await
    else { return; };

    let url = server.api_url(&format!("/mcp/servers/{}/ping", server_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("parse");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_http_ping_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/ping", random_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server");
}
