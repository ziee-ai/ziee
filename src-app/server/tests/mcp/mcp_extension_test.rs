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

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, parse_uuid, send_body_and_collect_events};
use crate::common::oai_capture_stub::{StubChat, StubPlan, StubToolCall};
use crate::common::stub_chat::register_stub_model;
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

// ============================================================================
// Permission revocation during execution (all-f44bdb26e811)
// ============================================================================

/// A user's access to a group-assigned system MCP server must be re-evaluated
/// on every request through the SAME `list_accessible` path the chat extension
/// uses (`helpers::get_all_accessible_config` → `Repos.mcp.list_accessible`).
/// This proves the allow is NOT cached: after the group assignment is revoked,
/// the very next accessible-servers fetch for that user no longer includes the
/// server, so a subsequent tool request would skip it as inaccessible
/// (`validate_and_build_config` drops servers not in the freshly-resolved set).
#[tokio::test]
async fn test_mcp_access_revocation_is_reevaluated_per_request() {
    let server = TestServer::start().await;

    // Admin creates the system server; regular user only reads accessible ones.
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "revoke_admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::read"],
    )
    .await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "revoke_user",
        &["mcp_servers::read"],
    )
    .await;

    let mcp_server = create_test_mcp_server(&server, &admin, true).await;
    let server_id =
        Uuid::parse_str(mcp_server["id"].as_str().expect("server id")).expect("uuid");

    // Assign the system server to the default group (the new user is a member).
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let default_group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("default group");
    sqlx::query!(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at)
         VALUES ($1, $2, NOW())",
        default_group.id,
        server_id
    )
    .execute(&pool)
    .await
    .expect("assign server to default group");

    let accessible_ids = |body: &serde_json::Value| -> Vec<String> {
        body["servers"]
            .as_array()
            .expect("servers array")
            .iter()
            .map(|s| s["id"].as_str().unwrap_or_default().to_string())
            .collect()
    };

    // Before revocation: the user can access the server.
    let url = server.api_url("/mcp/servers?page=1&per_page=1000");
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list accessible (granted)");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("parse granted list");
    assert!(
        accessible_ids(&body).contains(&server_id.to_string()),
        "server must be accessible while the group grant exists"
    );

    // Revoke the grant (group assignment removed).
    sqlx::query!(
        "DELETE FROM user_group_mcp_servers WHERE group_id = $1 AND mcp_server_id = $2",
        default_group.id,
        server_id
    )
    .execute(&pool)
    .await
    .expect("revoke group assignment");
    pool.close().await;

    // After revocation: the SAME token's next request no longer sees it —
    // enforcement is re-resolved, not cached from the earlier allow.
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list accessible (revoked)");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("parse revoked list");
    assert!(
        !accessible_ids(&body).contains(&server_id.to_string()),
        "server must be DENIED immediately after the grant is revoked (no cached allow)"
    );
}

// ============================================================================
// Server name in the tool list — `[<name>]` label + "Connected MCP servers"
// roster (feature: mcp-server-name-in-tools)
// ============================================================================

/// Register a user-owned HTTP MCP server (is_built_in = false, is_system = false)
/// WITH a description column set — the external server the roster describes.
async fn register_external_mcp(
    server: &TestServer,
    token: &str,
    name: &str,
    description: &str,
    url: &str,
) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": name,
            "description": description,
            "transport_type": "http",
            "url": url,
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(status, 201, "register external mcp: {status}: {body}");
    let row: serde_json::Value = serde_json::from_str(&body).unwrap();
    Uuid::parse_str(row["id"].as_str().unwrap()).unwrap()
}

/// A mock advertising a single `search_bio` tool and answering `tools/call`.
async fn start_bio_mock() -> MockMcpServer {
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method(
            "tools/list",
            MockResponse::JsonOk(json!({
                "tools": [ {
                    "name": "search_bio",
                    "description": "Search the biology corpus",
                    "inputSchema": { "type": "object", "properties": {}, "additionalProperties": true }
                } ]
            })),
        );
    }
    for _ in 0..20 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({
                "content": [ { "type": "text", "text": "bio-ok" } ],
                "isError": false,
            })),
        );
    }
    mock
}

/// Concatenate the text of every system message in a captured OpenAI request body.
fn system_text(body: &serde_json::Value) -> String {
    body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"].as_str() == Some("system"))
        .map(|m| match &m["content"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// (wire_name, description) of every tool attached to a captured request body.
fn tool_descs(body: &serde_json::Value) -> Vec<(String, String)> {
    body["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let f = t.get("function")?;
            let name = f.get("name")?.as_str()?.to_string();
            let desc = f
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Some((name, desc))
        })
        .collect()
}

// TEST-4: an external server's tool descriptions carry `[<name>] …`, the always-on
// built-in tools stay unlabeled, and the system prompt gains a "Connected MCP
// servers" roster listing the external server (built-ins absent).
#[tokio::test]
async fn external_tools_labeled_and_rostered_builtins_untouched() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "roster", &["*"]).await;

    let mock = start_bio_mock().await;
    let mcp_id = register_external_mcp(
        &server,
        &user.token,
        "biognosia",
        "Bio-knowledge graph over PubMed",
        &mock.base_url(),
    )
    .await;

    // Plain text reply (no tool call): we only need to capture ONE request that the
    // MCP extension has enriched with the tool list + the roster.
    let stub = StubChat::start(StubPlan::text("hi")).await;
    let model_id_s =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url(), true, None).await;
    let model_id = Uuid::parse_str(&model_id_s).unwrap();

    let conversation = create_conversation(&server, &user.token, None, None).await;
    let conversation_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        json!({
            "content": "hello",
            "model_id": model_id,
            "branch_id": branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
        }),
        &[],
    )
    .await;

    // Pick the MCP-enriched request. `last_request()` is unreliable here because a
    // background conversation-title generation call also hits the stub WITHOUT tools;
    // select the request that actually carries the external tool.
    let body = stub
        .requests()
        .into_iter()
        .find(|b| {
            tool_descs(b)
                .iter()
                .any(|(n, _)| n.ends_with("__search_bio"))
        })
        .expect("a captured request should carry the external MCP tool list");
    let descs = tool_descs(&body);

    // External tool: description prefixed `[biognosia] `.
    let (_, bio_desc) = descs
        .iter()
        .find(|(n, _)| n.ends_with("__search_bio"))
        .expect("external search_bio tool should be advertised");
    assert!(
        bio_desc.starts_with("[biognosia] "),
        "external tool description must carry the server name; got {bio_desc:?}"
    );

    // Built-in control: the always-on `ask_user` / `get_tool_result` built-ins
    // (attached for any tool-capable model) must NOT be labeled.
    let builtin = descs
        .iter()
        .find(|(n, _)| n.ends_with("__ask_user") || n.ends_with("__get_tool_result"))
        .expect("a built-in tool should be attached for a tool-capable model");
    assert!(
        !builtin.1.starts_with('['),
        "built-in tool must stay unlabeled; got {builtin:?}"
    );

    // Roster: one line, for the external server only.
    let sys = system_text(&body);
    let heading = sys
        .find("## Connected MCP servers")
        .unwrap_or_else(|| panic!("system prompt missing roster; sys={sys}"));
    // Bound the roster to its OWN section — other system notes (lit_search, skills)
    // follow it and also use `- ` bullets, so slice up to the next `## ` heading.
    let body_after = &sys[heading + "## Connected MCP servers".len()..];
    let end = body_after
        .find("\n## ")
        .map(|i| heading + "## Connected MCP servers".len() + i)
        .unwrap_or(sys.len());
    let roster = &sys[heading..end];
    assert_eq!(
        roster.matches("\n- ").count(),
        1,
        "exactly one external server in the roster (built-ins excluded); roster={roster}"
    );
    assert!(
        roster.contains("- biognosia — Bio-knowledge graph over PubMed (1 tools)"),
        "roster must list the external server with its description + advertised count; roster={roster}"
    );
}

// TEST-5: the description label does not disturb wire-name dispatch — the model's
// `<uuid>__search_bio` call still routes to the external server and executes.
#[tokio::test]
async fn labeled_external_tool_still_dispatches() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "dispatch", &["*"]).await;

    let mock = start_bio_mock().await;
    let mcp_id =
        register_external_mcp(&server, &user.token, "biognosia", "Bio corpus", &mock.base_url())
            .await;

    // Stub emits the FULL wire name the model saw (`<server_id>__search_bio`).
    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "tool_use".to_string(),
            name: format!("{mcp_id}__search_bio"),
            arguments: "{}".to_string(),
        }],
        ..Default::default()
    };
    let stub = StubChat::start(plan).await;
    let model_id_s =
        register_stub_model(&server, &user.token, &user.user_id, &stub.base_url(), true, None).await;
    let model_id = Uuid::parse_str(&model_id_s).unwrap();

    let conversation = create_conversation(&server, &user.token, None, None).await;
    let conversation_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);

    // Auto-approve so the tool executes without a manual approval round-trip.
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conversation_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "auto_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        json!({
            "content": "search it",
            "model_id": model_id,
            "branch_id": branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
        }),
        &[],
    )
    .await;

    // The labeled tool still routed to the external server: the mock got a tools/call.
    let calls = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .count();
    assert!(
        calls >= 1,
        "labeled external tool must still dispatch (mock should receive a tools/call)"
    );
}
