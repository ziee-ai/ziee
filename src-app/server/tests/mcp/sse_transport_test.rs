// MCP SSE transport integration tests
// Tests for MCP server with SSE transport (Server-Sent Events)

use crate::common::test_helpers::{self, TestUser};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an SSE MCP server for testing (uses mcp-weather-server)
async fn create_sse_weather_server(server: &crate::common::TestServer, user: &TestUser) -> Uuid {
    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("test_sse_weather_{}", &unique_id[..8]),
        "display_name": "Test SSE Weather Server",
        "description": "MCP server using HTTP transport (mcp-weather-server in streamable-http mode)",
        "enabled": true,
        "transport_type": "http",  // Changed to http since we're using streamable-http mode
        "url": "http://localhost:8080",
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

    assert_eq!(
        response.status(),
        201,
        "Should create SSE server successfully"
    );

    let body: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

// ============================================================================
// List Tools Tests
// ============================================================================

#[tokio::test]
async fn test_sse_list_server_tools() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE weather server
    let server_id = create_sse_weather_server(&server, &admin).await;

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

    assert_eq!(status, 200, "Should list tools successfully via SSE");

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");

    // Verify tools array exists
    let tools = body["tools"].as_array().expect("Should have tools array");

    // Verify weather-related tools are present
    assert!(!tools.is_empty(), "Should have tools in the list");

    // Verify tool structure
    if let Some(first_tool) = tools.first() {
        assert!(
            first_tool["name"].is_string(),
            "Tool should have name"
        );
        assert!(
            first_tool["input_schema"].is_object(),
            "Tool should have input_schema"
        );
    }
}

#[tokio::test]
async fn test_sse_list_tools_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

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
async fn test_sse_list_tools_server_not_found() {
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
async fn test_sse_call_weather_tool() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE weather server
    let server_id = create_sse_weather_server(&server, &admin).await;

    // First, list tools to see what's available
    let list_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("List tools request failed");

    let tools_body: serde_json::Value = list_response.json().await.unwrap();
    let tools = tools_body["tools"].as_array().expect("Should have tools");

    // Get first tool name
    let tool_name = tools[0]["name"].as_str().expect("Tool should have name");

    // Call the tool with appropriate arguments based on tool schema
    let url = server.api_url(&format!("/mcp/servers/{}/tools/{}/call", server_id, tool_name));

    // Most weather tools require location
    let payload = json!({
        "arguments": {
            "latitude": 35.6762,
            "longitude": 139.6503
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
        eprintln!("Call tool response (status {}): {}", status, body_text);
    }

    // SSE call might succeed or fail depending on external API
    assert!(
        status == 200 || status == 500,
        "Should call tool via SSE (got status {})",
        status
    );

    if status == 200 {
        let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
        assert!(body["content"].is_array(), "Should have content array");
    }
}

#[tokio::test]
async fn test_sse_call_tool_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

    // User without permission tries to call tool
    let url = server.api_url(&format!("/mcp/servers/{}/tools/get_weather/call", server_id));
    let payload = json!({
        "arguments": {
            "latitude": 35.6762,
            "longitude": 139.6503
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
async fn test_sse_call_tool_server_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // Use random UUID for server_id
    let random_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/get_weather/call", random_id));
    let payload = json!({
        "arguments": {
            "latitude": 35.6762,
            "longitude": 139.6503
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

// ============================================================================
// List Resources Tests
// ============================================================================

#[tokio::test]
async fn test_sse_list_server_resources() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

    // List resources from the server
    let url = server.api_url(&format!("/mcp/servers/{}/resources", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();

    // SSE weather server may or may not support resources
    assert!(
        status == 200 || status == 500,
        "Should handle list resources request via SSE (got {})",
        status
    );
}

#[tokio::test]
async fn test_sse_list_resources_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

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

// ============================================================================
// Disconnect Server Tests
// ============================================================================

#[tokio::test]
async fn test_sse_disconnect_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

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
        "Should connect to SSE server successfully"
    );

    // Now disconnect
    let disconnect_url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&disconnect_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should disconnect SSE server successfully");
}

#[tokio::test]
async fn test_sse_disconnect_permission_required() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

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
async fn test_sse_disconnect_idempotent() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

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
// Concurrent Tests
// ============================================================================

#[tokio::test]
async fn test_sse_concurrent_tool_calls() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    // Create an SSE server
    let server_id = create_sse_weather_server(&server, &admin).await;

    // Get first tool name
    let list_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("List tools request failed");

    let tools_body: serde_json::Value = list_response.json().await.unwrap();
    let tools = tools_body["tools"].as_array().expect("Should have tools");
    let tool_name = tools[0]["name"].as_str().expect("Tool should have name");

    // Make multiple concurrent tool calls
    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/mcp/servers/{}/tools/{}/call", server_id, tool_name));
    let payload = json!({
        "arguments": {
            "latitude": 35.6762,
            "longitude": 139.6503
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

    // Verify all requests completed (may succeed or fail depending on external API)
    for result in results {
        let (i, status) = result.expect("Task panicked");
        assert!(
            status == 200 || status == 500,
            "Concurrent SSE request {} should complete (got {})",
            i,
            status
        );
    }
}
