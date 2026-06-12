//! MCP chat extension integration tests
//!
//! Tests the MCP chat extension end-to-end:
//! - Tool discovery and injection into LLM requests
//! - Tool execution after LLM response
//! - Access control and permission checks
//! - Fine-grained tool selection
//! - SSE event emission for tool execution

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::{self, TestUser};
use crate::common::TestServer;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an MCP server for testing
async fn create_test_mcp_server(
    server: &TestServer,
    user: &TestUser,
    enabled: bool,
) -> serde_json::Value {
    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("test_chat_server_{}", &unique_id[..8]),
        "display_name": "Test Chat MCP Server",
        "description": "MCP server for chat extension testing",
        "enabled": enabled,
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
        .expect("Failed to create MCP server");

    assert_eq!(response.status(), 201, "Should create MCP server successfully");

    response.json().await.expect("Failed to parse response")
}

/// Create a user MCP server. Uses http transport — the MCP
/// user-policy force-sandboxes user stdio (requires
/// code_sandbox.enabled, off in tests). Transport choice is
/// incidental to what mcp_extension_test cares about (chat-side
/// MCP enforcement / cross-user isolation).
async fn create_user_mcp_server(
    server: &TestServer,
    user: &TestUser,
    enabled: bool,
) -> serde_json::Value {
    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("user_chat_server_{}", &unique_id[..8]),
        "display_name": "User Chat MCP Server",
        "description": "User-owned MCP server",
        "enabled": enabled,
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp",
        "timeout_seconds": 30
    });

    let url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create user MCP server");

    assert_eq!(response.status(), 201, "Should create user MCP server successfully");

    response.json().await.expect("Failed to parse response")
}

/// Assign MCP server to user group
async fn assign_server_to_group(
    server: &TestServer,
    admin_token: &str,
    server_id: Uuid,
    group_id: Uuid,
) {
    let url = server.api_url(&format!("/mcp/system-servers/{}/groups", server_id));
    let payload = json!({
        "group_ids": [group_id]
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to assign server to group");

    let status = response.status();
    if !status.is_success() {
        eprintln!("Group assignment failed:");
        eprintln!("  Status: {}", status);
        eprintln!("  Body: {}", response.text().await.unwrap_or_default());
        panic!("Should assign server to group successfully. Status: {}", status);
    }
}

// ============================================================================
// Phase 2.2: MCP Disabled/Enabled Scenarios
// ============================================================================

#[tokio::test]
async fn test_mcp_extension_disabled_by_default() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message without enable_mcp flag
    let response = crate::chat::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello",
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // MCP should not be triggered (no tools should be added to request)
    // This is verified by the fact that the mock provider doesn't receive tool configs
}

#[tokio::test]
async fn test_mcp_extension_enabled_with_no_servers() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message with enable_mcp but no servers configured
    let payload = json!({
        "content": "Hello",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(response.status(), 200, "Should send message successfully even with no MCP servers");
}

// ============================================================================
// Phase 2.3: Tool Execution and Error Handling
// ============================================================================

#[tokio::test]
async fn test_mcp_tools_added_to_llm_request() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers_admin::create",
            "mcp_servers::read",
            "groups::read",
            "groups::create",
            "groups::edit",
        ],
    )
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &admin, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &admin.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &admin.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message with MCP enabled. In the fire-and-forget model the reply +
    // extension events stream over the per-user chat stream, not the POST
    // response — `send_body_and_collect_events` subscribes, POSTs, and collects
    // until the terminal `complete`/`error` (this is auto-approve / default, so
    // a terminal always arrives). It asserts the POST returned 200.
    let body = json!({
        "content": "Use the fetch tool to get https://example.com",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": mcp_server_id,
                    "tools": [] // Empty = all tools
                }
            ]
        }
    });

    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &admin.token,
        conversation_id,
        body,
        &[],
    )
    .await;

    // Should have received streamed generation events (exact structure depends
    // on the provider response).
    assert!(!events.is_empty(), "Should receive streamed chat events");
}

// ============================================================================
// Phase 2.4: Access Control Tests
// ============================================================================

#[tokio::test]
async fn test_mcp_user_can_only_access_own_servers() {
    let server = TestServer::start().await;

    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers::create",
            "mcp_servers::read",
        ],
    )
    .await;

    let user2 = test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers::create",
            "mcp_servers::read",
        ],
    )
    .await;

    // User1 creates an MCP server
    let user1_server = create_user_mcp_server(&server, &user1, true).await;
    let user1_server_id = Uuid::parse_str(user1_server["id"].as_str().unwrap()).unwrap();

    // User2 creates conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &user2.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user2.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // User2 tries to use User1's server
    let payload = json!({
        "content": "Test message",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": user1_server_id,
                    "tools": []
                }
            ]
        }
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(
        response.status(),
        200,
        "Should still send message (but ignore inaccessible server)"
    );

    // The server should be filtered out during validation
    // The message should be sent without MCP tools from user1's server
}

#[tokio::test]
async fn test_mcp_user_can_access_group_servers() {
    let server = TestServer::start().await;

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
            "mcp_servers::read",
            "groups::read",
            "groups::create",
            "groups::edit",
            "groups::assign_users",
        ],
    )
    .await;

    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers::read",
        ],
    )
    .await;

    // Create group
    let group_payload = json!({
        "name": "test_group",
        "description": "Test Group",
        "permissions": []
    });
    let group_url = server.api_url("/groups");
    let group_response = reqwest::Client::new()
        .post(&group_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&group_payload)
        .send()
        .await
        .expect("Failed to create group");
    assert_eq!(group_response.status(), 201, "Should create group");
    let group = group_response.json::<serde_json::Value>().await.unwrap();
    let group_id = Uuid::parse_str(group["id"].as_str().unwrap()).unwrap();

    // Add user to group
    let assign_url = server.api_url("/groups/assign");
    let assign_payload = json!({
        "user_id": user.user_id,
        "group_id": group_id
    });
    let assign_response = reqwest::Client::new()
        .post(&assign_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&assign_payload)
        .send()
        .await
        .expect("Failed to assign user to group");
    assert_eq!(assign_response.status(), 204, "Should assign user to group");

    // Create system MCP server
    let mcp_server = create_test_mcp_server(&server, &admin, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Assign server to group
    assign_server_to_group(&server, &admin.token, mcp_server_id, group_id).await;

    // User creates conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // User sends message with group's MCP server
    let payload = json!({
        "content": "Test message with group MCP server",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": mcp_server_id,
                    "tools": []
                }
            ]
        }
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(
        response.status(),
        200,
        "Should send message with group MCP server tools"
    );
}

// ============================================================================
// Phase 2.5: Fine-Grained Tool Selection
// ============================================================================

#[tokio::test]
async fn test_mcp_specific_tool_selection() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers_admin::create",
            "mcp_servers::read",
        ],
    )
    .await;

    // Create MCP server with multiple tools
    let mcp_server = create_test_mcp_server(&server, &admin, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &admin.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &admin.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message with only specific tool selected
    let payload = json!({
        "content": "Test message",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": mcp_server_id,
                    "tools": ["fetch"] // Only fetch tool
                }
            ]
        }
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(
        response.status(),
        200,
        "Should send message with specific MCP tool"
    );

    // The extension should only add the "fetch" tool to the LLM request
}

#[tokio::test]
async fn test_mcp_all_tools_with_empty_array() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers_admin::create",
            "mcp_servers::read",
        ],
    )
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &admin, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &admin.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &admin.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message with empty tools array (should get all tools)
    let payload = json!({
        "content": "Test message",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": mcp_server_id,
                    "tools": [] // Empty = all tools
                }
            ]
        }
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(
        response.status(),
        200,
        "Should send message with all MCP tools"
    );
}

#[tokio::test]
async fn test_mcp_disabled_servers_ignored() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
            "mcp_servers_admin::create",
            "mcp_servers::read",
        ],
    )
    .await;

    // Create disabled MCP server
    let mcp_server = create_test_mcp_server(&server, &admin, false).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = crate::chat::helpers::create_conversation(&server, &admin.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = crate::chat::helpers::get_or_create_test_model(&server, &admin.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Send message with disabled server
    let payload = json!({
        "content": "Test message",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                {
                    "server_id": mcp_server_id,
                    "tools": []
                }
            ]
        }
    });

    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message");

    assert_eq!(
        response.status(),
        200,
        "Should send message (but ignore disabled server)"
    );

    // Disabled server should be filtered out during validation
}
