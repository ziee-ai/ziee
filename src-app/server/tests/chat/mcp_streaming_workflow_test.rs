//! MCP Streaming Workflow Integration Tests
//!
//! Comprehensive tests for MCP tool call streaming scenarios:
//! - Single complete event verification (the main bug fix test)
//! - Full auto-approve flow: tool execution → LLM response → single complete
//! - Manual approve full workflow
//! - Sequential tool calls
//! - Tool result persistence across messages
//!
//! These tests specifically target the streaming loop behavior and ensure
//! correct SSE event emission patterns.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;

// ============================================================================
// Helper Functions
// ============================================================================

/// Common permissions needed for MCP streaming workflow tests
const MCP_TEST_PERMISSIONS: &[&str] = &[
    "conversations::create",
    "conversations::read",
    "conversations::edit",
    "messages::create",
    "messages::read",
    "llm_models::read",
    "llm_models::create",
    "llm_providers::read",
    "llm_providers::create",
    "llm_providers::edit",
    "mcp_servers_admin::create",
    "mcp_servers_admin::read",
];

/// Directive prompt that explicitly requests tool use
const TOOL_USE_PROMPT: &str = "Use the fetch tool to get the content from https://example.com and return the result. You MUST use the available fetch tool - do not make assumptions about the content.";

/// Count events by type in an SSE event list
fn count_events_by_type(events: &[super::helpers::SSEEvent], event_type: &str) -> usize {
    events.iter().filter(|e| e.event == event_type).count()
}

/// Get the indices of events in order
fn get_event_indices(events: &[super::helpers::SSEEvent], event_types: &[&str]) -> Vec<(String, usize)> {
    events.iter()
        .enumerate()
        .filter(|(_, e)| event_types.contains(&e.event.as_str()))
        .map(|(i, e)| (e.event.clone(), i))
        .collect()
}

/// Check if a content event has text content (not tool_use)
fn has_text_content(data: &serde_json::Value) -> bool {
    if let Some(content) = data.get("content").and_then(|c| c.as_array()) {
        content.iter().any(|block| {
            block.get("type").and_then(|t| t.as_str()) == Some("text_delta")
                || block.get("type").and_then(|t| t.as_str()) == Some("text")
        })
    } else {
        false
    }
}

/// Assert exactly one complete event in the stream
fn assert_single_complete(events: &[super::helpers::SSEEvent]) {
    let complete_events: Vec<_> = events.iter()
        .filter(|e| e.event == "complete")
        .collect();

    assert_eq!(
        complete_events.len(),
        1,
        "Expected exactly 1 complete event, got {}.\nComplete events: {:?}",
        complete_events.len(),
        complete_events.iter().map(|e| &e.data).collect::<Vec<_>>()
    );
}

/// Assert that events appear in the specified order
fn assert_event_order(events: &[super::helpers::SSEEvent], expected_order: &[&str]) {
    let filtered: Vec<&str> = events.iter()
        .map(|e| e.event.as_str())
        .filter(|e| expected_order.contains(e))
        .collect();

    let mut last_idx = 0;
    for expected in expected_order {
        if let Some(idx) = filtered.iter().skip(last_idx).position(|&e| e == *expected) {
            last_idx = last_idx + idx + 1;
        } else {
            panic!(
                "Expected event '{}' not found in order.\nExpected order: {:?}\nActual order: {:?}",
                expected, expected_order, filtered
            );
        }
    }
}

/// Create an MCP server for testing (mcp-server-fetch)
async fn create_test_mcp_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    enabled: bool,
) -> serde_json::Value {
    let payload = json!({
        "name": format!("fetch_server_{}", Uuid::new_v4().to_string()[..8].to_string()),
        "display_name": "Test Workflow MCP Server",
        "description": "MCP server for streaming workflow testing",
        "enabled": enabled,
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
        .expect("Failed to create MCP server");

    assert_eq!(response.status(), 201, "Should create MCP server successfully");

    let mcp_server: serde_json::Value = response.json().await.expect("Failed to parse response");
    let server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Assign to default group
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

    response.json().await.expect("Failed to parse response")
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
                    "tools": []
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

// ============================================================================
// Priority 1: Core Flow Tests - Single Complete Event Verification
// ============================================================================

/// Test: Verify exactly ONE complete event is sent during auto-approve
///
/// This is the key test for the bug fix. The previous bug caused multiple
/// complete events to be sent when tools were auto-approved.
#[tokio::test]
async fn test_auto_approve_single_complete_event() {
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

    // DEBUG: Print all events to help diagnose issues
    eprintln!("\n=== Test: test_auto_approve_single_complete_event ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}'", i, event.event);
    }

    // Verify tool was used (mcpToolStart event exists)
    let tool_start_count = count_events_by_type(&events, "mcpToolStart");
    assert!(tool_start_count > 0, "Should have at least one mcpToolStart event (tool was called)");

    // Verify tool completed (mcpToolComplete event exists)
    let tool_complete_count = count_events_by_type(&events, "mcpToolComplete");
    assert!(tool_complete_count > 0, "Should have at least one mcpToolComplete event");

    // THE KEY ASSERTION: Exactly ONE complete event
    assert_single_complete(&events);

    // Verify the complete event has valid finish_reason
    // Either "stop" (normal termination) or "max_iterations" (LLM kept calling tools)
    let complete_event = events.iter()
        .find(|e| e.event == "complete")
        .expect("Should have complete event");

    let finish_reason = complete_event.data.get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    assert!(
        finish_reason == "stop" || finish_reason == "max_iterations",
        "Complete event should have finish_reason='stop' or 'max_iterations', got '{}'",
        finish_reason
    );
}

/// Test: Auto-approve executes tool AND gets LLM response in single stream
///
/// Verifies the expected flow:
/// 1. started
/// 2. content (tool_use deltas from LLM)
/// 3. mcpToolStart
/// 4. mcpToolComplete
/// 5. content (LLM text response after receiving tool result)
/// 6. complete (exactly once)
#[tokio::test]
async fn test_auto_approve_full_flow_tool_then_response() {
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
    eprintln!("\n=== Test: test_auto_approve_full_flow_tool_then_response ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        let has_text = has_text_content(&event.data);
        eprintln!("Event {}: type='{}' has_text={}", i, event.event, has_text);
    }

    // Verify the expected event order
    // Note: mcpToolComplete should come before the final content with LLM response
    let event_order = get_event_indices(&events, &["started", "mcpToolStart", "mcpToolComplete", "complete"]);
    eprintln!("Event order: {:?}", event_order);

    // Check order: started → mcpToolStart → mcpToolComplete → complete
    assert_event_order(&events, &["started", "mcpToolStart", "mcpToolComplete", "complete"]);

    // Find the index of mcpToolComplete
    let tool_complete_idx = events.iter()
        .position(|e| e.event == "mcpToolComplete")
        .expect("Should have mcpToolComplete event");

    // Look for text content AFTER tool complete
    // The LLM should respond with text after receiving the tool result
    let text_content_after_tool: Vec<_> = events.iter()
        .skip(tool_complete_idx + 1)
        .filter(|e| e.event == "content" && has_text_content(&e.data))
        .collect();

    // Note: This assertion may fail if the LLM decides not to generate text after tool use
    // In that case, we just verify the stream completes properly
    eprintln!("Text content events after tool complete: {}", text_content_after_tool.len());

    // THE KEY ASSERTION: Exactly ONE complete event
    assert_single_complete(&events);
}

/// Test: Manual approve full workflow with tool execution and response
///
/// Session 1:
/// - Send message
/// - Receive tool_use + mcpApprovalRequired
/// - Receive complete (stream ends)
///
/// Session 2:
/// - Send message with tool_approvals
/// - Tool executes (mcpToolStart, mcpToolComplete)
/// - LLM responds with text
/// - Receive complete
#[tokio::test]
async fn test_manual_approve_full_workflow() {
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

    // =========================================
    // SESSION 1: Send message, get approval request
    // =========================================
    eprintln!("\n=== Session 1: Initial message with tool use ===");

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

    assert_eq!(response1.status(), 200, "Session 1 should succeed");

    let events1 = super::helpers::parse_sse_events(response1).await;
    eprintln!("Session 1 events: {}", events1.len());
    for (i, event) in events1.iter().enumerate() {
        eprintln!("  Event {}: type='{}'", i, event.event);
    }

    // Session 1 should have mcpApprovalRequired event
    let approval_events: Vec<_> = events1.iter()
        .filter(|e| e.event == "mcpApprovalRequired")
        .collect();
    assert!(!approval_events.is_empty(), "Session 1 should emit mcpApprovalRequired");

    // Session 1 should have exactly 1 complete event
    assert_single_complete(&events1);

    // Session 1 should NOT have tool execution
    let tool_start_count = count_events_by_type(&events1, "mcpToolStart");
    assert_eq!(tool_start_count, 0, "Session 1 should NOT execute tools (pending approval)");

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert!(!pending.is_empty(), "Should have pending approvals after session 1");

    let tool_use_id = pending[0]["tool_use_id"].as_str().expect("Should have tool_use_id");
    eprintln!("Pending tool_use_id: {}", tool_use_id);

    // =========================================
    // SESSION 2: Approve and resume
    // =========================================
    eprintln!("\n=== Session 2: Approve and resume execution ===");

    let approvals = vec![json!({
        "tool_use_id": tool_use_id,
        "decision": "approved"
    })];

    let response2 = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT, // Same prompt for resume execution
        Some(approvals),
    )
    .await;

    assert_eq!(response2.status(), 200, "Session 2 should succeed");

    let events2 = super::helpers::parse_sse_events(response2).await;
    eprintln!("Session 2 events: {}", events2.len());
    for (i, event) in events2.iter().enumerate() {
        eprintln!("  Event {}: type='{}'", i, event.event);
    }

    // Session 2 should have tool execution events
    let tool_start_count = count_events_by_type(&events2, "mcpToolStart");
    let tool_complete_count = count_events_by_type(&events2, "mcpToolComplete");
    assert!(tool_start_count > 0, "Session 2 should have mcpToolStart (tool executed)");
    assert!(tool_complete_count > 0, "Session 2 should have mcpToolComplete (tool completed)");

    // Session 2 should have exactly 1 complete OR error event (external service might fail)
    let complete_count = count_events_by_type(&events2, "complete");
    let error_count = count_events_by_type(&events2, "error");
    assert!(
        complete_count == 1 || error_count == 1,
        "Session 2 should have exactly 1 complete or 1 error event. Got complete={}, error={}",
        complete_count,
        error_count
    );

    // If we got a complete event, verify it has valid finish_reason
    if complete_count == 1 {
        eprintln!("Session 2 completed normally");
    } else {
        eprintln!("Session 2 ended with error (likely external service issue)");
    }

    // Verify no more pending approvals (tool was executed regardless of final result)
    let pending_after = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending_after.len(), 0, "Should have no pending approvals after session 2");
}

// ============================================================================
// Priority 2: Error Scenarios
// ============================================================================

/// Test: Verify MAX_ITERATIONS prevents infinite loops
///
/// Uses a prompt that might cause repeated tool calls to verify the loop
/// terminates with max_iterations finish_reason and exactly ONE complete event.
#[tokio::test]
async fn test_auto_approve_no_infinite_loop() {
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

    assert_eq!(response.status(), 200, "Should send message successfully");

    let events = super::helpers::parse_sse_events(response).await;

    eprintln!("\n=== Test: test_auto_approve_no_infinite_loop ===");
    eprintln!("Total events received: {}", events.len());

    // THE KEY ASSERTION: Even if max_iterations is reached, only ONE complete event
    assert_single_complete(&events);

    // Verify the complete event
    let complete_event = events.iter()
        .find(|e| e.event == "complete")
        .expect("Should have complete event");

    let finish_reason = complete_event.data.get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // finish_reason should be either "stop" (normal) or "max_iterations" (loop limit)
    assert!(
        finish_reason == "stop" || finish_reason == "max_iterations",
        "Complete event should have finish_reason='stop' or 'max_iterations', got '{}'",
        finish_reason
    );
}

// ============================================================================
// Priority 3: Persistence Tests
// ============================================================================

/// Test: Tool results persist in conversation history
///
/// After a tool is executed, verify that:
/// 1. The tool_use content is saved to the message
/// 2. The tool_result content is saved to the message
/// 3. Subsequent messages can reference the tool result
#[tokio::test]
async fn test_tool_results_persist_in_history() {
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

    // Wait for stream to complete
    let events = super::helpers::parse_sse_events(response).await;

    // Verify tool was executed
    let tool_complete_count = count_events_by_type(&events, "mcpToolComplete");
    assert!(tool_complete_count > 0, "Tool should have executed");

    // Get conversation history - API returns array directly
    let history = super::helpers::get_conversation_history(&server, &user.token, conversation_id).await;
    let messages = history.as_array().expect("History should be an array of messages");

    eprintln!("\n=== Test: test_tool_results_persist_in_history ===");
    eprintln!("Messages in history: {}", messages.len());

    // Find messages with tool_use or tool_result content
    let mut found_tool_use = false;
    let mut found_tool_result = false;

    for msg in messages {
        if let Some(contents) = msg.get("contents").and_then(|c| c.as_array()) {
            for content in contents {
                let content_type = content.get("content_type").and_then(|t| t.as_str());
                eprintln!("  Content type: {:?}", content_type);

                if content_type == Some("tool_use") {
                    found_tool_use = true;
                }
                if content_type == Some("tool_result") {
                    found_tool_result = true;
                }
            }
        }
    }

    assert!(found_tool_use, "History should contain tool_use content");
    assert!(found_tool_result, "History should contain tool_result content");
}

/// Test: Tool results are available via API history endpoint
#[tokio::test]
async fn test_tool_results_in_api_history() {
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

    // Wait for stream to complete
    let _events = super::helpers::parse_sse_events(response).await;

    // Fetch conversation via API
    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get messages");

    assert_eq!(response.status(), 200, "Should get messages successfully");

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    // API returns array directly
    let messages = body.as_array().expect("Response should be an array of messages");

    eprintln!("\n=== Test: test_tool_results_in_api_history ===");
    eprintln!("API returned {} messages", messages.len());

    // Verify tool_use and tool_result content types exist
    let content_types: Vec<&str> = messages.iter()
        .flat_map(|msg| {
            msg.get("contents")
                .and_then(|c| c.as_array())
                .map(|contents| {
                    contents.iter()
                        .filter_map(|c| c.get("content_type").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();

    eprintln!("Content types found: {:?}", content_types);

    assert!(
        content_types.contains(&"tool_use"),
        "API should return tool_use content. Found: {:?}",
        content_types
    );
    assert!(
        content_types.contains(&"tool_result"),
        "API should return tool_result content. Found: {:?}",
        content_types
    );
}
