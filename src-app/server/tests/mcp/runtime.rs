// MCP runtime integration tests
// Tests for MCP server runtime operations (tools, resources, disconnect)

use crate::common::test_helpers::{self, TestUser};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a fetch MCP server for testing
async fn create_fetch_server(server: &crate::common::TestServer, user: &TestUser) -> Uuid {
    use uuid::Uuid;
    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("test_fetch_server_{}", &unique_id[..8]),
        "display_name": "Test Fetch Server",
        "description": "MCP server for fetching web content",
        "enabled": true,
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"],
        "timeout_seconds": 60
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create server");

    assert_eq!(
        response.status(),
        201,
        "Should create fetch server successfully"
    );

    let body: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

/// Create a disabled server for testing
async fn create_disabled_server(server: &crate::common::TestServer, user: &TestUser) -> Uuid {
    let payload = json!({
        "name": "disabled_server",
        "display_name": "Disabled Server",
        "description": "Disabled test server",
        "enabled": false,
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"],
        "timeout_seconds": 30
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create server");

    let body: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

// ============================================================================
// List Tools Tests
// ============================================================================

#[tokio::test]
async fn test_list_server_tools() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

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

    if status != 200 {
        eprintln!("Error response (status {}): {}", status, body_text);
    }

    assert_eq!(status, 200, "Should list tools successfully");

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");

    // Verify tools array exists
    let tools = body["tools"].as_array().expect("Should have tools array");

    // Verify fetch tool is present
    let has_fetch = tools.iter().any(|t| t["name"].as_str() == Some("fetch"));
    assert!(has_fetch, "Should have fetch tool in the list");

    // Verify tool structure
    if let Some(fetch_tool) = tools.iter().find(|t| t["name"].as_str() == Some("fetch")) {
        assert!(
            fetch_tool["description"].is_string(),
            "Tool should have description"
        );
        assert!(
            fetch_tool["input_schema"].is_object(),
            "Tool should have input_schema"
        );
    }
}

#[tokio::test]
async fn test_list_tools_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

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
async fn test_list_tools_server_not_found() {
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
async fn test_call_fetch_tool() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // Call fetch tool with a URL
    let url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
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

    if status != 200 {
        eprintln!("Error response (status {}): {}", status, body_text);
    }

    assert_eq!(status, 200, "Should call tool successfully");

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");

    // Verify response structure
    assert!(body["content"].is_array(), "Should have content array");
    assert_eq!(body["is_error"], false, "Should not be an error");

    // Verify content is returned
    let content = body["content"].as_array().unwrap();
    assert!(!content.is_empty(), "Should have content in response");
}

#[tokio::test]
async fn test_call_tool_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // User without permission tries to call tool
    let url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

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
async fn test_call_tool_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Use random UUID for server_id
    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", random_id));
    let payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server (more secure - doesn't reveal if server exists)");
}

#[tokio::test]
async fn test_call_tool_with_invalid_arguments() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // Call fetch tool with invalid arguments (missing required 'url' field)
    let url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let payload = json!({
        "arguments": {
            "invalid_field": "value"
        }
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // Should return error (either 400 or the tool returns is_error: true)
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // MCP servers might handle this as an error in content or return 400
    assert!(
        status == 400 || body["is_error"] == true,
        "Should handle invalid arguments"
    );
}

// ============================================================================
// List Resources Tests
// ============================================================================

#[tokio::test]
async fn test_list_server_resources() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // List resources from the server
    let url = server.api_url(&format!("/mcp/servers/{}/resources", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");

    // The fetch server doesn't support resources, so it may return an error
    // Either 200 with empty array OR 500 with "Method not found" is acceptable
    assert!(
        status == 200 || status == 500,
        "Should handle list resources request (got {})",
        status
    );

    if status == 200 {
        let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
        // Verify resources array exists
        assert!(
            body["resources"].is_array(),
            "Should have resources array"
        );
    } else {
        // Server doesn't support resources (Method not found is acceptable)
        assert!(
            body_text.contains("Method not found") || body_text.contains("SYSTEM_INTERNAL_ERROR"),
            "Should return appropriate error for unsupported method"
        );
    }
}

#[tokio::test]
async fn test_list_resources_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // User without permission tries to list resources
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

#[tokio::test]
async fn test_list_resources_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Use random UUID for server_id
    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/resources", random_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server (more secure - doesn't reveal if server exists)");
}

// ============================================================================
// Read Resource Tests
// ============================================================================

#[tokio::test]
async fn test_read_server_resource_endpoint_exists() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // Try to read a resource (fetch server may not have resources, but endpoint should work)
    let url = server.api_url(&format!("/mcp/servers/{}/resources/read", server_id));
    let payload = json!({
        "uri": "file:///nonexistent"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // Should return 200 or appropriate error (not 404 for endpoint itself)
    assert!(
        response.status() != 404 || response.status() == 200 || response.status() == 400,
        "Endpoint should exist (got {})",
        response.status()
    );
}

#[tokio::test]
async fn test_read_resource_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // User without permission tries to read resource
    let url = server.api_url(&format!("/mcp/servers/{}/resources/read", server_id));
    let payload = json!({
        "uri": "file:///test"
    });

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

// ============================================================================
// Disconnect Server Tests
// ============================================================================

#[tokio::test]
async fn test_disconnect_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // First, connect to the server by listing tools
    let list_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        list_response.status(),
        200,
        "Should connect to server successfully"
    );

    // Now disconnect
    let disconnect_url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&disconnect_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should disconnect successfully");
}

#[tokio::test]
async fn test_disconnect_server_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // User without permission tries to disconnect
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
async fn test_disconnect_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Use random UUID for server_id
    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/disconnect", random_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should return 403 for inaccessible server (more secure - doesn't reveal if server exists)");
}

#[tokio::test]
async fn test_disconnect_idempotent() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // Disconnect once
    let url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response1 = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response1.status(), 200, "First disconnect should succeed");

    // Disconnect again (should be idempotent)
    let response2 = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response2.status(),
        200,
        "Second disconnect should also succeed (idempotent)"
    );
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_disabled_server_operations() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a disabled server
    let server_id = create_disabled_server(&server, &admin).await;

    // Try to list tools from disabled server
    let url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    // Should either succeed (runtime checks enabled status) or return error
    // The actual behavior depends on implementation
    assert!(
        response.status() == 200 || response.status() == 400 || response.status() == 500,
        "Should handle disabled server (got {})",
        response.status()
    );
}

#[tokio::test]
async fn test_concurrent_tool_calls() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a fetch server
    let server_id = create_fetch_server(&server, &admin).await;

    // Make multiple concurrent tool calls
    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

    let mut handles = vec![];
    for i in 0..3 {
        let client = client.clone();
        let url = url.clone();
        let payload = payload.clone();
        let token = admin.token.clone();

        let handle = tokio::spawn(async move {
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&payload)
                .send()
                .await
                .expect(&format!("Request {} failed", i));

            (i, response.status())
        });

        handles.push(handle);
    }

    // Wait for all requests to complete
    let results = futures::future::join_all(handles).await;

    // Verify all requests succeeded
    for result in results {
        let (i, status) = result.expect("Task panicked");
        assert_eq!(
            status, 200,
            "Concurrent request {} should succeed",
            i
        );
    }
}

// ============================================================================
// Access Control Tests
// ============================================================================

#[tokio::test]
async fn test_runtime_user_cannot_access_other_user_server_tools() {
    let server = crate::common::TestServer::start().await;
    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["mcp_servers::read", "mcp_servers::create"],
    )
    .await;
    let user2 = test_helpers::create_user_with_permissions(&server, "user2", &["mcp_servers::read"])
        .await;

    // User1 creates a personal server
    let payload = json!({
        "name": "user1_private_server",
        "display_name": "User1 Private Server",
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"],
        "timeout_seconds": 30
    });

    let create_url = server.api_url("/mcp/servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // User2 tries to list tools on User1's server - should get 403
    let list_tools_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&list_tools_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User2 should not be able to list tools on User1's server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // User2 tries to call a tool on User1's server - should get 403
    let call_tool_url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let call_payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

    let response = reqwest::Client::new()
        .post(&call_tool_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&call_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User2 should not be able to call tools on User1's server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // User2 tries to list resources on User1's server - should get 403
    let list_resources_url = server.api_url(&format!("/mcp/servers/{}/resources", server_id));
    let response = reqwest::Client::new()
        .get(&list_resources_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User2 should not be able to list resources on User1's server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // User2 tries to read a resource on User1's server - should get 403
    let read_resource_url = server.api_url(&format!("/mcp/servers/{}/resources/read", server_id));
    let read_payload = json!({
        "uri": "file:///some/path"
    });

    let response = reqwest::Client::new()
        .post(&read_resource_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&read_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User2 should not be able to read resources on User1's server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // User2 tries to disconnect User1's server - should get 403
    let disconnect_url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&disconnect_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User2 should not be able to disconnect User1's server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");
}

#[tokio::test]
async fn test_runtime_user_can_access_group_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::edit"],
    )
    .await;

    // Create a regular user with mcp_servers::read permission
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Get a group to assign the user to (use the default group)
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let group_result = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let group_id = group_result.id;

    // Note: User is already assigned to default group by create_user_with_permissions
    // but we'll ensure it's there (idempotent operation)
    let user_uuid = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    sqlx::query(
        "INSERT INTO user_groups (user_id, group_id, assigned_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT DO NOTHING",
    )
    .bind(user_uuid)
    .bind(group_id)
    .execute(&pool)
    .await
    .expect("Failed to assign user to group");

    pool.close().await;

    // Admin creates a system server
    let server_id = create_fetch_server(&server, &admin).await;

    // Admin assigns the system server to the group
    let assign_url = server.api_url(&format!("/mcp/system-servers/{}/groups", server_id));
    let assign_payload = json!({
        "group_ids": [group_id]
    });

    let assign_response = reqwest::Client::new()
        .post(&assign_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&assign_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        assign_response.status(),
        204,
        "Should assign server to group"
    );

    // User (who is in the group) should be able to list tools
    let list_tools_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&list_tools_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User in group should be able to list tools on group-assigned system server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body["tools"].is_array(), "Should return tools array");

    // User should be able to call a tool
    let call_tool_url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let call_payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

    let response = reqwest::Client::new()
        .post(&call_tool_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&call_payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    if status != 200 {
        let error_body = response.text().await.expect("Failed to read error body");
        eprintln!("Error calling tool: {} - {}", status, error_body);
    }

    assert_eq!(
        status,
        200,
        "User in group should be able to call tools on group-assigned system server"
    );
}

#[tokio::test]
async fn test_runtime_user_cannot_access_unassigned_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create a regular user with mcp_servers::read permission
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Admin creates a system server but does NOT assign it to any group
    let server_id = create_fetch_server(&server, &admin).await;

    // User tries to list tools on the unassigned system server - should get 403
    let list_tools_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&list_tools_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User should not be able to list tools on unassigned system server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // User tries to call a tool - should get 403
    let call_tool_url = server.api_url(&format!("/mcp/servers/{}/tools/fetch/call", server_id));
    let call_payload = json!({
        "arguments": {
            "url": "https://example.com"
        }
    });

    let response = reqwest::Client::new()
        .post(&call_tool_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&call_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User should not be able to call tools on unassigned system server"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "USER_NO_ACCESS");

    // Admin users with mcp_servers_admin::* permissions can access ALL servers
    // This allows admins to debug and manage servers without needing group assignments
    let admin_response = reqwest::Client::new()
        .get(&list_tools_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        admin_response.status(),
        200,
        "Admin with mcp_servers_admin::* permissions can access unassigned system servers"
    );

    let admin_body: serde_json::Value = admin_response.json().await.expect("Failed to parse JSON");
    assert!(admin_body["tools"].is_array(), "Admin should get tools list");
}
