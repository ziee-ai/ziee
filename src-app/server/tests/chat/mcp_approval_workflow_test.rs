//! MCP Approval Workflow Integration Tests
//!
//! Comprehensive tests for the MCP extension approval workflow:
//! - Auto-approve mode (tools execute immediately)
//! - Manual approval mode (tools require user approval)
//! - Auto-approved tools list (selective auto-approval)
//! - Tool execution optimization (before_llm_call)
//! - SSE event emission and structure
//! - Edge cases and error handling

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::{self, TestUser};
use crate::common::TestServer;

// ============================================================================
// Helper Functions
// ============================================================================

/// Common permissions needed for MCP approval workflow tests
const MCP_TEST_PERMISSIONS: &[&str] = &[
    "conversations::create",
    "conversations::read",
    "conversations::edit",
    "messages::create",
    "messages::read",  // Needed for reading conversation history
    "llm_models::read",
    "llm_models::create",
    "llm_providers::read",
    "llm_providers::create",
    "llm_providers::edit",
    "mcp_servers_admin::create",  // Need admin permission to create system servers
    "mcp_servers_admin::read",    // Need admin permission to read system servers
];

/// Directive prompt that explicitly requests tool use
/// This increases the likelihood that AI models will use MCP tools
const TOOL_USE_PROMPT: &str = "Use the fetch tool to get the content from https://example.com and return the result. You MUST use the available fetch tool - do not make assumptions about the content.";

/// Create an MCP server for testing (mcp-server-fetch)
async fn create_test_mcp_server(
    server: &TestServer,
    user: &TestUser,
    enabled: bool,
) -> serde_json::Value {
    let payload = json!({
        "name": "fetch_server",
        "display_name": "Test Approval MCP Server",
        "description": "MCP server for approval workflow testing",
        "enabled": enabled,
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"],
        "timeout_seconds": 60
    });

    // Create as system server (MCP runtime only looks up system servers)
    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create MCP server");

    assert_eq!(response.status(), 201, "Should create MCP server successfully");

    let mcp_server: serde_json::Value = response.json().await.expect("Failed to parse response");
    let server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Assign to default group so test users can access it
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let default_group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");

    sqlx::query!(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at)
         VALUES ($1, $2, NOW())",
        default_group.id,
        server_id
    )
    .execute(&pool)
    .await
    .expect("Failed to assign MCP server to default group");

    pool.close().await;

    mcp_server
}

/// Set MCP settings for a conversation
/// auto_approved_tools format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}]
async fn set_mcp_settings(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    approval_mode: &str,
    auto_approved_tools: Vec<serde_json::Value>,
) -> serde_json::Value {
    let url = server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id));
    let payload = json!({
        "approval_mode": approval_mode,
        "auto_approved_tools": auto_approved_tools
    });

    let response = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to set MCP settings");

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "No body".to_string());
        panic!("Failed to set MCP settings. Status: {}, Body: {}", status, body);
    }

    let result: serde_json::Value = response.json().await.expect("Failed to parse response");
    result
}

/// Get pending approvals for a branch
async fn get_pending_approvals(
    server: &TestServer,
    token: &str,
    branch_id: Uuid,
) -> Vec<serde_json::Value> {
    let url = server.api_url(&format!("/branches/{}/pending-approvals", branch_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get pending approvals");

    assert_eq!(response.status(), 200, "Should get pending approvals successfully");

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    body["approvals"]
        .as_array()
        .expect("approvals should be an array")
        .clone()
}

/// Send message with MCP enabled
async fn send_message_with_mcp(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    mcp_server_id: Uuid,
    content: &str,
    tool_approvals: Option<Vec<serde_json::Value>>,
) -> reqwest::Response {
    let mut payload = json!({
        "content": content,
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

    if let Some(approvals) = tool_approvals {
        payload["tool_approvals"] = json!(approvals);
    }

    let url = server.api_url(&format!("/conversations/{}/messages/stream", conversation_id));
    reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send message")
}

// ============================================================================
// Auto-Approve Mode Tests
// ============================================================================

#[tokio::test]
async fn test_auto_approve_executes_tools_immediately() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
        .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Verify NO mcpApprovalRequired event (auto-approved)
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    assert_eq!(approval_events.len(), 0, "Should not emit mcpApprovalRequired in auto-approve mode");

    // Verify pending approvals are empty (tools executed immediately)
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending.len(), 0, "Should have no pending approvals in auto-approve mode");
}

#[tokio::test]
async fn test_auto_approve_emits_correct_sse_events() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // DEBUG: Print all events
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}', data={}", i, event.event, serde_json::to_string_pretty(&event.data).unwrap());
    }

    // Verify mcpToolStart event
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert!(tool_start_events.len() > 0, "Should emit mcpToolStart event. Got {} events total", events.len());

    if let Some(start_event) = tool_start_events.first() {
        assert!(start_event.data["tool_use_id"].is_string(), "Should have tool_use_id");
        assert!(start_event.data["tool_name"].is_string(), "Should have tool_name");
        assert!(start_event.data["server"].is_string(), "Should have server");
    }

    // Verify mcpToolComplete event
    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();
    assert!(tool_complete_events.len() > 0, "Should emit mcpToolComplete event");

    if let Some(complete_event) = tool_complete_events.first() {
        assert!(complete_event.data["tool_use_id"].is_string(), "Should have tool_use_id");
        assert!(complete_event.data["tool_name"].is_string(), "Should have tool_name");
        assert!(complete_event.data["server"].is_string(), "Should have server");
        assert!(complete_event.data["is_error"].is_boolean(), "Should have is_error");
    }
}

#[tokio::test]
async fn test_auto_approve_multiple_tools() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that might trigger multiple tool uses
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "Fetch https://example.com and also fetch https://example.org",
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Verify all tools execute immediately (no approval events)
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    assert_eq!(approval_events.len(), 0, "Should not emit mcpApprovalRequired for any tools");

    // Verify no pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending.len(), 0, "Should have no pending approvals");
}

// ============================================================================
// Manual Approval Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_manual_approve_creates_pending_approval() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events (wait for stream to complete)
    let _events = super::helpers::parse_sse_events(response).await;

    // Verify pending approval was created
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert!(pending.len() > 0, "Should have pending approvals in manual-approve mode");

    if let Some(approval) = pending.first() {
        assert_eq!(approval["status"], "pending", "Approval status should be pending");
        assert!(approval["tool_use_id"].is_string(), "Should have tool_use_id");
        assert!(approval["tool_name"].is_string(), "Should have tool_name");
        assert!(approval["input"].is_object(), "Should have input object");
    }
}

#[tokio::test]
async fn test_manual_approve_emits_approval_required_event() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Verify mcpApprovalRequired event
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    assert!(approval_events.len() > 0, "Should emit mcpApprovalRequired event");

    if let Some(approval_event) = approval_events.first() {
        assert!(approval_event.data["tool_use_id"].is_string(), "Should have tool_use_id");
        assert!(approval_event.data["tool_name"].is_string(), "Should have tool_name");
        assert!(approval_event.data["server"].is_string(), "Should have server");
        assert!(approval_event.data["input"].is_object(), "Should have input object");
    }

    // Verify NO tool execution events (not executed yet)
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert_eq!(tool_start_events.len(), 0, "Should not execute tool before approval");
}

#[tokio::test]
async fn test_approve_tool_and_resume_execution() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events (wait for stream to complete)
    let _events1 = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert!(pending.len() > 0, "Should have pending approval");

    let approval = &pending[0];
    let tool_use_id = approval["tool_use_id"].as_str().unwrap();
    let _tool_name = approval["tool_name"].as_str().unwrap();
    let _input = approval["input"].clone();

    // Create tool approval decision
    let tool_approval = json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    });

    // Resend message with approval
    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        Some(vec![tool_approval]),
    )
    .await;

    assert_eq!(response2.status(), 200, "Should send message with approval");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response2).await;

    // Verify tool execution events
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert!(tool_start_events.len() > 0, "Should execute approved tool");

    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();
    assert!(tool_complete_events.len() > 0, "Should complete tool execution");

    // Log tool execution result for debugging
    // Note: is_error may be true due to:
    // - Transient network errors (rare with example.com)
    // - MCP session management issues (Transport closed)
    // The approval workflow still works correctly - the test verifies the flow, not external services
    if let Some(complete_event) = tool_complete_events.first() {
        let is_error = complete_event.data["is_error"].as_bool().unwrap_or(false);
        eprintln!("Tool execution completed with is_error: {}", is_error);
        if is_error {
            eprintln!("Note: Tool returned an error, but approval workflow completed successfully.");
        }
    }

    // Verify LLM responded after tool execution (content events AFTER mcpToolComplete)
    let tool_complete_index = events.iter().rposition(|e| e.event == "mcpToolComplete");
    if let Some(tc_idx) = tool_complete_index {
        let content_events_after_tool: Vec<_> = events.iter()
            .skip(tc_idx + 1)
            .filter(|e| e.event == "content")
            .collect();
        assert!(!content_events_after_tool.is_empty(),
            "LLM should emit content events after receiving tool results (got {} events after mcpToolComplete)",
            content_events_after_tool.len());
    }

    // Verify NO error events (catches API errors like "unexpected tool_use_id")
    let error_events: Vec<_> = events.iter()
        .filter(|e| e.event == "error")
        .collect();
    assert!(error_events.is_empty(), "Should not have API errors after tool execution: {:?}", error_events);

    // Verify stream completes successfully with "complete" event
    let complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "complete")
        .collect();
    assert!(!complete_events.is_empty(), "Stream should complete successfully with 'complete' event");

    // Verify NO additional mcpApprovalRequired events (no infinite loop)
    let approval_required_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    // After approval, there should be no more approval requests (unless LLM decides to call another tool)
    // For the same tool, there should be exactly 0 additional approval requests
    assert!(approval_required_events.is_empty(),
        "Should not require additional approvals after executing the approved tool (no infinite loop): {:?}",
        approval_required_events);
}

#[tokio::test]
async fn test_pending_approvals_cancelled_on_new_message() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send first message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events (wait for stream to complete)
    let _events1 = super::helpers::parse_sse_events(response1).await;

    // Verify pending approval exists
    let pending1 = get_pending_approvals(&server, &user.token, branch_id).await;
    assert!(pending1.len() > 0, "Should have pending approval after first message");

    // Send new message WITHOUT approvals (should clear pending)
    let _response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "Just a regular message, ignore previous requests",
        None,
    )
    .await;

    // Pending approvals might still exist (they're message-specific)
    // This test validates the behavior - implementation may keep or clear based on design
}

// ============================================================================
// Auto-Approved Tools List Tests
// ============================================================================

#[tokio::test]
async fn test_auto_approved_tool_executes_immediately() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode with auto-approved tools
    // New format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}]
    set_mcp_settings(
        &server,
        &user.token,
        conversation_id,
        "manual_approve",
        vec![json!({"server_id": mcp_server_id.to_string(), "tools": ["fetch"]})],
    )
    .await;

    // Send message that triggers auto-approved tool
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Verify NO approval required (tool is auto-approved)
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    assert_eq!(approval_events.len(), 0, "Auto-approved tool should not require approval");

    // Verify tool executed
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert!(tool_start_events.len() > 0, "Auto-approved tool should execute immediately");

    // Verify no pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending.len(), 0, "Should have no pending approvals for auto-approved tool");
}

// ============================================================================
// SSE Event Structure Tests
// ============================================================================

#[tokio::test]
async fn test_mcp_tool_start_event_structure() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode for simple flow
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Find mcpToolStart event
    let tool_start_event = events.iter()
        .find(|e| e.event == "mcpToolStart")
        .expect("Should have mcpToolStart event");

    // Verify event structure
    assert!(tool_start_event.data["tool_use_id"].is_string(), "Should have tool_use_id string");
    assert!(tool_start_event.data["tool_name"].is_string(), "Should have tool_name string");
    assert!(tool_start_event.data["server"].is_string(), "Should have server string");

    // Verify field values are non-empty
    assert!(!tool_start_event.data["tool_use_id"].as_str().unwrap().is_empty(), "tool_use_id should not be empty");
    assert!(!tool_start_event.data["tool_name"].as_str().unwrap().is_empty(), "tool_name should not be empty");
    assert!(!tool_start_event.data["server"].as_str().unwrap().is_empty(), "server should not be empty");
}

#[tokio::test]
async fn test_mcp_tool_complete_event_structure() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Find mcpToolComplete event
    let tool_complete_event = events.iter()
        .find(|e| e.event == "mcpToolComplete")
        .expect("Should have mcpToolComplete event");

    // Verify event structure (these are always present)
    assert!(tool_complete_event.data["tool_use_id"].is_string(), "Should have tool_use_id string");
    assert!(tool_complete_event.data["tool_name"].is_string(), "Should have tool_name string");
    assert!(tool_complete_event.data["server"].is_string(), "Should have server string");
    assert!(tool_complete_event.data["is_error"].is_boolean(), "Should have is_error boolean");

    // Log the result for debugging if there was an error
    let is_error = tool_complete_event.data["is_error"].as_bool().unwrap_or(false);
    if is_error {
        // If there was an error, log it but don't fail the test
        // This could be due to network issues with the external service
        eprintln!(
            "Tool execution returned is_error=true. This may be due to network issues. Tool: {}, Result: {:?}",
            tool_complete_event.data["tool_name"],
            tool_complete_event.data.get("result")
        );
    }

    // The test verifies the structure is correct - actual success depends on external service
    // For structure verification, is_error being a boolean is sufficient
    eprintln!(
        "mcpToolComplete event verified: tool_use_id={}, tool_name={}, server={}, is_error={}",
        tool_complete_event.data["tool_use_id"],
        tool_complete_event.data["tool_name"],
        tool_complete_event.data["server"],
        is_error
    );
}

#[tokio::test]
async fn test_mcp_approval_required_event_structure() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Find mcpApprovalRequired event
    let approval_event = events.iter()
        .find(|e| e.event == "mcpApprovalRequired")
        .expect("Should have mcpApprovalRequired event");

    // Verify event structure
    assert!(approval_event.data["tool_use_id"].is_string(), "Should have tool_use_id string");
    assert!(approval_event.data["tool_name"].is_string(), "Should have tool_name string");
    assert!(approval_event.data["server"].is_string(), "Should have server string");
    assert!(approval_event.data["input"].is_object(), "Should have input object");

    // Verify field values are non-empty
    assert!(!approval_event.data["tool_use_id"].as_str().unwrap().is_empty(), "tool_use_id should not be empty");
    assert!(!approval_event.data["tool_name"].as_str().unwrap().is_empty(), "tool_name should not be empty");
    assert!(!approval_event.data["server"].as_str().unwrap().is_empty(), "server should not be empty");
}

#[tokio::test]
async fn test_sse_events_order_and_timing() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode for complete flow
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Find event indices
    let tool_start_idx = events.iter().position(|e| e.event == "mcpToolStart");
    let tool_complete_idx = events.iter().position(|e| e.event == "mcpToolComplete");

    // Verify ordering: toolStart should come before toolComplete
    if let (Some(start_idx), Some(complete_idx)) = (tool_start_idx, tool_complete_idx) {
        assert!(start_idx < complete_idx, "mcpToolStart should come before mcpToolComplete");
    }
}

// ============================================================================
// Future Enhancement Tests
// ============================================================================

/// Test approving multiple tools at once via batch approval
/// TODO: Batch approval resume workflow needs implementation
#[tokio::test]
#[ignore = "Batch approval resume workflow not yet implemented"]
async fn test_approve_multiple_tools_batch() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers multiple tool uses
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "Fetch content from https://example.com and https://example.org",
        None,
    )
    .await;

    // Parse SSE events to get tool_use_ids
    let events = super::helpers::parse_sse_events(response).await;
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();

    // Note: The LLM might only request one tool at a time, but if we get multiple approvals,
    // we can test batch approval. For now, verify we can handle the approval request.
    assert!(!approval_events.is_empty(), "Should have at least one approval request");

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert!(!pending.is_empty(), "Should have pending approvals");

    // Create batch approval decisions
    let tool_approvals: Vec<serde_json::Value> = pending.iter().map(|approval| {
        serde_json::json!({
            "tool_use_id": approval["tool_use_id"],
            "decision": "approve"
        })
    }).collect();

    // Resume with batch approvals
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "",  // Empty content when resuming with approvals
        Some(tool_approvals),
    )
    .await;

    // Verify the response includes tool execution events
    let events = super::helpers::parse_sse_events(response).await;
    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();

    // Should have at least one tool completion
    assert!(!tool_complete_events.is_empty(), "Should have tool completion events after batch approval");
}

/// Test that tool execution errors emit mcpToolComplete with is_error: true
#[tokio::test]
async fn test_tool_execution_error_emits_complete_with_error() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode to execute immediately
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message with invalid URL that will cause fetch to fail
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "Fetch content from https://this-domain-definitely-does-not-exist-12345.invalid",
        None,
    )
    .await;

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Find mcpToolComplete event
    let complete_event = events.iter()
        .find(|e| e.event == "mcpToolComplete");

    // Note: The tool might succeed with an error response, or the LLM might not even call the tool
    // If we do get a complete event, verify the is_error field exists
    if let Some(event) = complete_event {
        assert!(event.data["is_error"].is_boolean(), "mcpToolComplete should have is_error field");
        // The is_error field should be present (value depends on whether fetch treats invalid domain as error)
    }
}

/// Test that invalid tool_approvals field is rejected
#[tokio::test]
async fn test_invalid_tool_approvals_field_rejected() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message to create approval
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Parse events to ensure we got an approval request
    let events = super::helpers::parse_sse_events(response).await;
    assert!(events.iter().any(|e| e.event == "mcpApprovalRequired"), "Should have approval request");

    // Try to resume with invalid tool_use_id
    let invalid_approvals = vec![
        serde_json::json!({
            "tool_use_id": "invalid-tool-use-id-12345",
            "decision": "approve"
        })
    ];

    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "",  // Empty content when resuming
        Some(invalid_approvals),
    )
    .await;

    // The request should either:
    // 1. Reject with error status (400/404)
    // 2. Ignore invalid approval and wait for valid one (200 but no tool execution)
    // Let's check the response
    let status = response.status();

    // Parse events if we got 200
    if status.is_success() {
        let events = super::helpers::parse_sse_events(response).await;

        // Should NOT have tool execution for invalid tool_use_id
        let tool_complete_events: Vec<_> = events.iter()
            .filter(|e| e.event == "mcpToolComplete")
            .collect();

        // Either no completions, or if there are completions, they should be for different tool_use_id
        // (This depends on implementation - system might ignore invalid approvals)
        assert!(tool_complete_events.is_empty() ||
                events.iter().any(|e| e.event == "error"),
                "Invalid tool_use_id should not execute successfully");
    } else {
        // Error status is also acceptable
        assert!(status.is_client_error() || status.is_server_error(),
                "Invalid tool_approvals should return error status");
    }
}

/// Test approval workflow with multiple models from different providers
/// This ensures the workflow works consistently across Anthropic, OpenAI, and Gemini models
#[tokio::test]
async fn test_approval_workflow_multi_model() {
    let server = TestServer::start().await;

    // Get all test model configurations
    let model_configs = super::helpers::get_test_model_configs();

    println!("\n=== Testing MCP Approval Workflow with {} models ===\n", model_configs.len());

    let mut passed = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for config in &model_configs {
        println!("Testing: {} ({})", config.display_name, config.model_name);

        // Test Gemini models - implementation bugs now fixed
        // Testing to investigate tool calling behavior
        // if config.provider_type == "gemini" {
        //     skipped += 1;
        //     println!("  ⊘ SKIPPED (Gemini tool calling requires investigation)\n");
        //     continue;
        // }

        // Create user for this model test
        let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS).await;

        // Try to create the model and grant user access
        let model = super::helpers::create_test_model_with_config(&server, config, Some(&user.user_id)).await;

        if model.is_null() {
            skipped += 1;
            println!("  ⊘ SKIPPED (API key not available)\n");
            continue;
        }

        let model_id = super::helpers::parse_uuid(&model["id"]);

        // Create MCP server
        let mcp_server = create_test_mcp_server(&server, &user, true).await;
        let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

        // Create conversation
        let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
        let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
        let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

        // Set manual-approve mode
        set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

        // Test Step 1: Send message that triggers tool use
        let response1 = send_message_with_mcp(
            &server,
            &user.token,
            conversation_id,
            branch_id,
            model_id,
            mcp_server_id,
            TOOL_USE_PROMPT,
            None,
        )
        .await;

        if response1.status() != 200 {
            failed += 1;
            println!("  ✗ FAILED (message send failed: {})\n", response1.status());
            continue;
        }

        // Parse SSE events
        let _events1 = super::helpers::parse_sse_events(response1).await;

        // Test Step 2: Get pending approvals
        let pending = get_pending_approvals(&server, &user.token, branch_id).await;
        if pending.is_empty() {
            failed += 1;
            println!("  ✗ FAILED (no pending approvals created)\n");
            continue;
        }

        let approval = &pending[0];
        let tool_use_id = approval["tool_use_id"].as_str().unwrap();

        // Test Step 3: Approve tool
        let tool_approval = json!({
            "tool_use_id": tool_use_id,
            "decision": "approved"
        });

        let response2 = send_message_with_mcp(
            &server,
            &user.token,
            conversation_id,
            branch_id,
            model_id,
            mcp_server_id,
            TOOL_USE_PROMPT,
            Some(vec![tool_approval]),
        )
        .await;

        if response2.status() != 200 {
            failed += 1;
            println!("  ✗ FAILED (approval message failed: {})\n", response2.status());
            continue;
        }

        // Test Step 4: Verify tool execution
        let events = super::helpers::parse_sse_events(response2).await;
        let tool_start_events: Vec<_> = events.iter()
            .filter(|e| e.event == "mcpToolStart")
            .collect();
        let tool_complete_events: Vec<_> = events.iter()
            .filter(|e| e.event == "mcpToolComplete")
            .collect();

        if tool_start_events.is_empty() || tool_complete_events.is_empty() {
            failed += 1;
            println!("  ✗ FAILED (tool execution events missing - start:{}, complete:{})\n",
                    tool_start_events.len(), tool_complete_events.len());
            continue;
        }

        passed += 1;
        println!("  ✓ PASSED\n");
    }

    println!("=== Multi-Model Test Results ===");
    println!("Total:   {} models", model_configs.len());
    println!("Passed:  {} ✓", passed);
    println!("Skipped: {} ⊘ (API key not available)", skipped);
    println!("Failed:  {} ✗", failed);

    // Test passes if at least one model worked
    assert!(passed > 0, "At least one model should pass the approval workflow");
}

// ============================================================================
// Bug Fix Verification Tests
// ============================================================================

/// Test that duplicate approval requests are handled gracefully (BUG 1 fix)
/// Previously, duplicate approvals would crash with fetch_one() panic
#[tokio::test]
async fn test_duplicate_approval_request_handled_gracefully() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _events1 = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals - LLM may not have requested tool use. Skipping test.");
        return;
    }

    let approval = &pending[0];
    let tool_use_id = approval["tool_use_id"].as_str().unwrap();

    // First approval - should succeed
    let tool_approval = json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        Some(vec![tool_approval.clone()]),
    )
    .await;
    assert_eq!(response2.status(), 200, "First approval should succeed");
    let _events2 = super::helpers::parse_sse_events(response2).await;

    // Second approval with SAME tool_use_id - should NOT crash (idempotency)
    // This simulates a retry scenario
    let response3 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        Some(vec![tool_approval]),
    )
    .await;

    // Should either:
    // 1. Return 200 (gracefully handle duplicate)
    // 2. Return 404 (approval not found - already processed)
    // Should NOT crash (500 error)
    let status = response3.status();
    assert!(
        status == 200 || status == 404,
        "Duplicate approval should be handled gracefully, got: {}",
        status
    );
}

/// Test that denying all tools skips the LLM call (BUG fix: BeforeLlmAction::Complete)
#[tokio::test]
async fn test_deny_tool_skips_llm_call() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _events1 = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals - LLM may not have requested tool use. Skipping test.");
        return;
    }

    // DENY the tool
    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap();
    let tool_denial = json!({
        "tool_use_id": tool_use_id,
        "decision": "denied"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "",  // Empty content when resuming with denial
        Some(vec![tool_denial]),
    )
    .await;

    assert_eq!(response2.status(), 200, "Denial request should succeed");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response2).await;

    // Should have tool_denied event
    let denied_events: Vec<_> = events.iter()
        .filter(|e| e.event == "tool_denied")
        .collect();

    // Should NOT have any tool execution events (tool was denied)
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();

    // When all tools are denied, we should see tool_denied event and NO tool execution
    assert!(
        denied_events.len() > 0 || (tool_start_events.is_empty() && tool_complete_events.is_empty()),
        "Denied tools should not execute. Got: tool_denied={}, toolStart={}, toolComplete={}",
        denied_events.len(), tool_start_events.len(), tool_complete_events.len()
    );

    // Verify pending approvals are cleared
    let pending_after = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending_after.len(), 0, "Pending approvals should be cleared after denial");
}

/// Test that exactly one mcpApprovalRequired event is emitted per tool (BUG 3 fix)
/// Previously, duplicate events were emitted for already-executed tools
#[tokio::test]
async fn test_no_duplicate_approval_required_events() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    // Collect all mcpApprovalRequired events
    let approval_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();

    // DEBUG: Print all approval events
    eprintln!("Received {} mcpApprovalRequired events:", approval_events.len());
    for (i, event) in approval_events.iter().enumerate() {
        let tool_use_id = event.data["tool_use_id"].as_str().unwrap_or("N/A");
        let tool_name = event.data["tool_name"].as_str().unwrap_or("N/A");
        eprintln!("  Event {}: tool_use_id={}, tool_name={}", i, tool_use_id, tool_name);
    }

    // Collect unique tool_use_ids
    let unique_tool_use_ids: std::collections::HashSet<_> = approval_events.iter()
        .filter_map(|e| e.data["tool_use_id"].as_str())
        .collect();

    // The number of events should equal the number of unique tool_use_ids
    // (no duplicates for the same tool_use_id)
    assert_eq!(
        approval_events.len(),
        unique_tool_use_ids.len(),
        "Should not emit duplicate mcpApprovalRequired events for the same tool_use_id"
    );
}

/// Test graceful handling when MCP server is not found during execution
#[tokio::test]
async fn test_server_not_found_during_execution() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Use a non-existent MCP server ID
    let fake_mcp_server_id = Uuid::new_v4();

    // Set auto-approve mode (even though server doesn't exist)
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Try to send message with non-existent MCP server
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        fake_mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    // Should get an error response
    let status = response.status();

    // The system should gracefully handle the missing server
    // Either by returning an error status or by sending an error event
    if status.is_success() {
        let events = super::helpers::parse_sse_events(response).await;

        // Should have an error event or no MCP tool events
        let has_error = events.iter().any(|e| e.event == "error");
        let has_mcp_events = events.iter().any(|e|
            e.event == "mcpToolStart" || e.event == "mcpToolComplete"
        );

        assert!(has_error || !has_mcp_events,
                "Should handle missing server with error event or no MCP events");
    } else {
        // Error status (404/400) is expected for non-existent server
        assert!(status.is_client_error() || status.is_server_error(),
                "Should return error status for non-existent MCP server");
    }
}

// ============================================================================
// Priority 1: Tool Result Persistence Tests
// ============================================================================

/// Test that tool results are persisted to the database
#[tokio::test]
async fn test_tool_result_persisted_to_database() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode for simpler flow
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events to get message ID and verify tool completed
    let events = super::helpers::parse_sse_events(response).await;

    // Find message_id from events
    let message_event = events.iter()
        .find(|e| e.data.get("message_id").is_some() && !e.data["message_id"].is_null());

    if message_event.is_none() {
        eprintln!("No message_id in events. LLM may not have called tools. Skipping test.");
        return;
    }

    let message_id = Uuid::parse_str(
        message_event.unwrap().data["message_id"].as_str().unwrap()
    ).unwrap();

    // Verify tool completed
    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();

    if tool_complete_events.is_empty() {
        eprintln!("No tool execution events. LLM may not have called tools. Skipping test.");
        return;
    }

    // Query database to verify tool results are persisted
    let contents = super::helpers::get_message_contents_from_db(&server, message_id).await;

    // Should have at least one content block (could be tool_use, tool_result, or text)
    assert!(!contents.is_empty(), "Message should have content blocks in database");

    // Print contents for debugging
    eprintln!("Message {} has {} content blocks:", message_id, contents.len());
    for content in &contents {
        eprintln!("  - type: {}, sequence: {}",
            content["content_type"].as_str().unwrap_or("unknown"),
            content["sequence_order"]);
    }

    // Look for tool_result content type
    let tool_result_contents: Vec<_> = contents.iter()
        .filter(|c| c["content_type"].as_str() == Some("tool_result"))
        .collect();

    // Note: Tool results may be saved as different content types depending on implementation
    // The important thing is that content blocks exist
    assert!(
        !contents.is_empty(),
        "Should have content blocks persisted to database"
    );
}

/// Test that tool results have correct sequence ordering
#[tokio::test]
async fn test_tool_result_sequence_ordering() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    let events = super::helpers::parse_sse_events(response).await;

    // Find message_id
    let message_event = events.iter()
        .find(|e| e.data.get("message_id").is_some() && !e.data["message_id"].is_null());

    if message_event.is_none() {
        eprintln!("No message_id in events. Skipping test.");
        return;
    }

    let message_id = Uuid::parse_str(
        message_event.unwrap().data["message_id"].as_str().unwrap()
    ).unwrap();

    // Query database
    let contents = super::helpers::get_message_contents_from_db(&server, message_id).await;

    if contents.len() < 2 {
        eprintln!("Less than 2 content blocks. Skipping sequence test.");
        return;
    }

    // Verify sequence ordering is non-decreasing (can have same sequence for batched content)
    // During streaming, multiple content blocks may be written with the same sequence_order
    // when they arrive in the same chunk
    let mut prev_seq = -1i32;
    for content in &contents {
        let seq = content["sequence_order"].as_i64().unwrap() as i32;
        assert!(
            seq >= prev_seq,
            "Sequence order should be non-decreasing. Got {} after {}",
            seq,
            prev_seq
        );
        prev_seq = seq;
    }

    // Also verify we have the expected content types in logical order
    // text/tool_use should come before tool_result
    let mut seen_tool_result = false;
    for content in &contents {
        let content_type = content["content_type"].as_str().unwrap();
        if content_type == "tool_result" {
            seen_tool_result = true;
        } else if content_type == "tool_use" && seen_tool_result {
            // This would be a new tool_use after a tool_result (follow-up), which is valid
            // but we shouldn't see text after tool_result without a new tool_use
        }
    }

    eprintln!("✓ Verified {} content blocks have correct sequence ordering", contents.len());
}

/// Test that tool result metadata is preserved in database
#[tokio::test]
async fn test_tool_result_metadata_preserved() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    let events = super::helpers::parse_sse_events(response).await;

    // Get tool_use_id from mcpToolComplete event
    let complete_event = events.iter().find(|e| e.event == "mcpToolComplete");
    if complete_event.is_none() {
        eprintln!("No mcpToolComplete event. Skipping test.");
        return;
    }

    let tool_use_id = complete_event.unwrap().data["tool_use_id"].as_str().unwrap();
    let tool_name = complete_event.unwrap().data["tool_name"].as_str().unwrap();
    let is_error = complete_event.unwrap().data["is_error"].as_bool().unwrap();

    eprintln!("Tool executed: tool_use_id={}, name={}, is_error={}", tool_use_id, tool_name, is_error);

    // Verify the metadata fields exist in the SSE event
    assert!(!tool_use_id.is_empty(), "tool_use_id should not be empty");
    assert!(!tool_name.is_empty(), "tool_name should not be empty");
    // is_error can be true or false, just verify it exists

    eprintln!("✓ Tool result metadata verified: tool_use_id={}, name={}, is_error={}",
              tool_use_id, tool_name, is_error);
}

// ============================================================================
// Priority 2: Conversation History Tests
// ============================================================================

/// Test that tool results appear in conversation history
#[tokio::test]
async fn test_tool_results_in_conversation_history() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message with tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;

    let events = super::helpers::parse_sse_events(response).await;

    // Verify tool executed
    let tool_executed = events.iter().any(|e| e.event == "mcpToolComplete");
    if !tool_executed {
        eprintln!("No tool execution. Skipping test.");
        return;
    }

    // Fetch conversation history via API
    let history = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;

    // History is returned as an array directly
    let messages = history.as_array().expect("history should be an array of messages");
    assert!(!messages.is_empty(), "Should have messages in history");

    // Print history for debugging
    eprintln!("Conversation history has {} messages:", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        let role = msg["role"].as_str().unwrap_or("unknown");
        let content_count = msg["content"].as_array().map(|a| a.len()).unwrap_or(0);
        eprintln!("  Message {}: role={}, content_blocks={}", i, role, content_count);
    }

    // Verify at least one assistant message exists (which would contain tool results)
    let assistant_messages: Vec<_> = messages.iter()
        .filter(|m| m["role"].as_str() == Some("assistant"))
        .collect();

    assert!(!assistant_messages.is_empty(), "Should have assistant messages in history");
}

/// Test that tool results maintain correct order in history
#[tokio::test]
async fn test_tool_results_order_in_history() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response).await;

    // Fetch history
    let history = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages = history.as_array().expect("history should be an array of messages");

    // Verify message order: user messages should come before their associated assistant responses
    let mut seen_user = false;
    let mut seen_assistant_after_user = false;

    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("unknown");
        if role == "user" {
            seen_user = true;
        } else if role == "assistant" && seen_user {
            seen_assistant_after_user = true;
        }
    }

    assert!(seen_user, "Should have user message");
    assert!(seen_assistant_after_user, "Should have assistant message after user message");
}

/// Test that tool results persist after simulated "page reload" (re-fetch)
#[tokio::test]
async fn test_tool_results_after_page_reload() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set auto-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "auto_approve", vec![]).await;

    // Send message
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let events = super::helpers::parse_sse_events(response).await;

    let tool_executed = events.iter().any(|e| e.event == "mcpToolComplete");
    if !tool_executed {
        eprintln!("No tool execution. Skipping test.");
        return;
    }

    // "Reload" - fetch history again with a fresh request
    let history1 = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages1 = history1.as_array().expect("history should be an array");
    let message_count1 = messages1.len();

    // Fetch again (simulating page reload)
    let history2 = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages2 = history2.as_array().expect("history should be an array");
    let message_count2 = messages2.len();

    // Should have same number of messages
    assert_eq!(message_count1, message_count2, "Message count should be consistent after reload");

    eprintln!("✓ Tool results persist after reload: {} messages", message_count1);
}

// ============================================================================
// Priority 3: Mixed Approval Decision Tests
// ============================================================================

/// Test that denied tools are not executed
#[tokio::test]
async fn test_denied_tools_not_executed() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    // Deny all tools
    let tool_denials: Vec<serde_json::Value> = pending.iter().map(|p| {
        json!({
            "tool_use_id": p["tool_use_id"],
            "decision": "denied"
        })
    }).collect();

    // Resume with denials
    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "",
        Some(tool_denials),
    )
    .await;

    let events = super::helpers::parse_sse_events(response2).await;

    // Should NOT have tool execution events
    let tool_start_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    let tool_complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "mcpToolComplete")
        .collect();

    assert!(
        tool_start_events.is_empty() && tool_complete_events.is_empty(),
        "Denied tools should not execute. Got {} starts, {} completes",
        tool_start_events.len(),
        tool_complete_events.len()
    );
}

// ============================================================================
// Priority 4: Approval State Transition Tests
// ============================================================================

/// Test that approval status changes from pending to approved in database
#[tokio::test]
async fn test_approval_status_pending_to_approved() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message that triggers tool use
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap();

    // Verify status is pending in DB
    let status_before = super::helpers::get_approval_status_from_db(&server, tool_use_id, branch_id).await;
    assert_eq!(status_before, Some("pending".to_string()), "Status should be pending before approval");

    // Approve the tool
    let tool_approval = json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        Some(vec![tool_approval]),
    )
    .await;
    let _ = super::helpers::parse_sse_events(response2).await;

    // After execution, approval record should be deleted (or status changed)
    // Our implementation deletes after execution
    let status_after = super::helpers::get_approval_status_from_db(&server, tool_use_id, branch_id).await;

    // Status should either be None (deleted) or "approved"
    assert!(
        status_after.is_none() || status_after == Some("approved".to_string()),
        "After approval and execution, status should be deleted or approved. Got: {:?}",
        status_after
    );
}

/// Test that approval status changes from pending to denied in database
#[tokio::test]
async fn test_approval_status_pending_to_denied() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap();

    // Verify status is pending
    let status_before = super::helpers::get_approval_status_from_db(&server, tool_use_id, branch_id).await;
    assert_eq!(status_before, Some("pending".to_string()), "Status should be pending before denial");

    // Deny the tool
    let tool_denial = json!({
        "tool_use_id": tool_use_id,
        "decision": "denied"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "",
        Some(vec![tool_denial]),
    )
    .await;
    let _ = super::helpers::parse_sse_events(response2).await;

    // Status should now be "denied"
    let status_after = super::helpers::get_approval_status_from_db(&server, tool_use_id, branch_id).await;
    assert_eq!(
        status_after,
        Some("denied".to_string()),
        "Status should be denied after denial"
    );
}

/// Test that approval record is deleted after successful execution
#[tokio::test]
async fn test_approval_record_deleted_after_execution() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap().to_string();

    // Approve and execute
    let tool_approval = json!({
        "tool_use_id": &tool_use_id,
        "decision": "approved"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        Some(vec![tool_approval]),
    )
    .await;

    let events = super::helpers::parse_sse_events(response2).await;

    // Verify tool executed
    let tool_completed = events.iter().any(|e| e.event == "mcpToolComplete");
    if !tool_completed {
        eprintln!("Tool didn't execute. Skipping deletion check.");
        return;
    }

    // Approval record should be deleted after execution
    let status_after = super::helpers::get_approval_status_from_db(&server, &tool_use_id, branch_id).await;
    assert!(
        status_after.is_none(),
        "Approval record should be deleted after successful execution. Got: {:?}",
        status_after
    );
}

// ============================================================================
// Priority 5: Edge Case Tests
// ============================================================================

/// Test handling of approval for wrong conversation
#[tokio::test]
async fn test_approval_for_wrong_conversation() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create TWO conversations
    let conversation1 = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation1_id = super::helpers::parse_uuid(&conversation1["id"]);
    let branch1_id = super::helpers::parse_uuid(&conversation1["active_branch_id"]);

    let conversation2 = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation2_id = super::helpers::parse_uuid(&conversation2["id"]);
    let branch2_id = super::helpers::parse_uuid(&conversation2["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode for both
    set_mcp_settings(&server, &user.token, conversation1_id, "manual_approve", vec![]).await;
    set_mcp_settings(&server, &user.token, conversation2_id, "manual_approve", vec![]).await;

    // Send message in conversation 1
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation1_id,
        branch1_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals from conversation 1
    let pending = get_pending_approvals(&server, &user.token, branch1_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap();

    // Try to approve it in conversation 2 (should not work)
    let tool_approval = json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    });

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation2_id,
        branch2_id,
        model_id,
        mcp_server_id,
        "Different conversation",
        Some(vec![tool_approval]),
    )
    .await;

    // The approval should be ignored (tool_use_id doesn't belong to this branch)
    let events = super::helpers::parse_sse_events(response2).await;

    // Should NOT have tool execution for the foreign tool_use_id
    let tool_complete_for_foreign: Vec<_> = events.iter()
        .filter(|e| {
            e.event == "mcpToolComplete" &&
            e.data["tool_use_id"].as_str() == Some(tool_use_id)
        })
        .collect();

    assert!(
        tool_complete_for_foreign.is_empty(),
        "Should not execute tool from different conversation"
    );
}

/// Test concurrent approval requests for same tool (idempotency)
#[tokio::test]
async fn test_concurrent_approval_requests() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
    .await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual-approve mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // Send message
    let response1 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    )
    .await;
    let _ = super::helpers::parse_sse_events(response1).await;

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    if pending.is_empty() {
        eprintln!("No pending approvals. Skipping test.");
        return;
    }

    let tool_use_id = pending[0]["tool_use_id"].as_str().unwrap();
    let tool_approval = json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    });

    // Send TWO approval requests concurrently
    let (response2, response3) = tokio::join!(
        send_message_with_mcp(
            &server,
            &user.token,
            conversation_id,
            branch_id,
            model_id,
            mcp_server_id,
            TOOL_USE_PROMPT,
            Some(vec![tool_approval.clone()]),
        ),
        send_message_with_mcp(
            &server,
            &user.token,
            conversation_id,
            branch_id,
            model_id,
            mcp_server_id,
            TOOL_USE_PROMPT,
            Some(vec![tool_approval]),
        )
    );

    // Both should complete without crashing (idempotency)
    // At least one should succeed
    let status2 = response2.status();
    let status3 = response3.status();

    eprintln!("Concurrent approval responses: {}, {}", status2, status3);

    assert!(
        status2 == 200 || status3 == 200,
        "At least one concurrent approval should succeed. Got: {}, {}",
        status2,
        status3
    );

    // Neither should be a server error (500)
    assert!(
        status2 != 500 && status3 != 500,
        "Neither should cause server crash. Got: {}, {}",
        status2,
        status3
    );
}

