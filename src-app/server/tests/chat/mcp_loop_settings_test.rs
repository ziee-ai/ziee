//! MCP Loop Settings Integration Tests
//!
//! Tests for the loop settings feature that controls streaming iteration behavior:
//! - max_iteration: Hard limit on iterations
//! - stop_when_no_tool_calling: Stop when LLM doesn't call tools
//! - stop_when_tools_called: Stop when specific tools are called
//!
//! Note: force_final_answer and per_tool_max_iteration are defined but not yet
//! implemented in the backend, so tests for those are skipped.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;

// ============================================================================
// Helper Functions
// ============================================================================

/// Common permissions needed for MCP loop settings tests
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

/// Create an MCP server for testing (mcp-server-fetch)
async fn create_test_mcp_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    enabled: bool,
) -> serde_json::Value {
    let payload = json!({
        "name": format!("loop_test_server_{}", Uuid::new_v4().to_string()[..8].to_string()),
        "display_name": "Loop Settings Test MCP Server",
        "description": "MCP server for loop settings testing",
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

/// Set MCP settings for a conversation including loop_settings
async fn set_mcp_settings_with_loop(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    approval_mode: &str,
    loop_settings: Option<serde_json::Value>,
) -> serde_json::Value {
    let url = server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id));
    let mut payload = json!({
        "approval_mode": approval_mode,
        "auto_approved_tools": []
    });

    if let Some(ls) = loop_settings {
        payload["loop_settings"] = ls;
    }

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

/// Get MCP settings for a conversation
async fn get_mcp_settings(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
) -> serde_json::Value {
    let url = server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get MCP settings");

    assert!(response.status().is_success(), "Should get MCP settings successfully");

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
) -> reqwest::Response {
    let payload = json!({
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
// Test 1: API CRUD - Loop settings can be set and retrieved
// ============================================================================

/// Test that loop_settings can be set and retrieved via API
#[tokio::test]
async fn test_loop_settings_api_crud() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
        .await;

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Create a test UUID for stop_when_tools_called
    let test_server_id = Uuid::new_v4();

    // Set loop_settings with all fields
    let loop_settings = json!({
        "max_iteration": 5,
        "stop_when_no_tool_calling": false,
        "stop_when_tools_called": [
            { "server_id": test_server_id.to_string(), "tool_name": "finish_task" }
        ],
        "force_final_answer": true,
        "per_tool_max_iteration": [
            { "server_id": test_server_id.to_string(), "tool_name": "search", "max_iteration": 3 }
        ]
    });

    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", Some(loop_settings)).await;

    // Get and verify settings
    let settings = get_mcp_settings(&server, &user.token, conversation_id).await;

    eprintln!("\n=== Test: test_loop_settings_api_crud ===");
    eprintln!("Settings response: {}", serde_json::to_string_pretty(&settings).unwrap());

    let ls = &settings["settings"]["loop_settings"];

    // Verify all fields are persisted correctly
    assert_eq!(ls["max_iteration"], 5, "max_iteration should be 5");
    assert_eq!(ls["stop_when_no_tool_calling"], false, "stop_when_no_tool_calling should be false");
    assert_eq!(ls["force_final_answer"], true, "force_final_answer should be true");

    // Verify stop_when_tools_called array
    let stop_tools = ls["stop_when_tools_called"].as_array()
        .expect("stop_when_tools_called should be an array");
    assert_eq!(stop_tools.len(), 1, "Should have one stop tool");
    assert_eq!(stop_tools[0]["tool_name"], "finish_task");

    // Verify per_tool_max_iteration array
    let per_tool = ls["per_tool_max_iteration"].as_array()
        .expect("per_tool_max_iteration should be an array");
    assert_eq!(per_tool.len(), 1, "Should have one per-tool limit");
    assert_eq!(per_tool[0]["tool_name"], "search");
    assert_eq!(per_tool[0]["max_iteration"], 3);
}

// ============================================================================
// Test 2: Default values are applied when not explicitly set
// ============================================================================

/// Test that default loop_settings values are returned when not set
#[tokio::test]
async fn test_loop_settings_defaults_applied() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS)
        .await;

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Set MCP settings without loop_settings (should use defaults)
    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", None).await;

    // Get settings and verify defaults
    let settings = get_mcp_settings(&server, &user.token, conversation_id).await;

    eprintln!("\n=== Test: test_loop_settings_defaults_applied ===");
    eprintln!("Settings response: {}", serde_json::to_string_pretty(&settings).unwrap());

    let ls = &settings["settings"]["loop_settings"];

    // Verify default values (from LoopSettings::default())
    assert_eq!(ls["stop_when_no_tool_calling"], true, "Default stop_when_no_tool_calling should be true");
    assert_eq!(ls["max_iteration"], 10, "Default max_iteration should be 10");
    assert_eq!(ls["force_final_answer"], false, "Default force_final_answer should be false");

    // Verify empty arrays for stop_when_tools_called and per_tool_max_iteration
    let stop_tools = ls["stop_when_tools_called"].as_array()
        .expect("stop_when_tools_called should be an array");
    assert_eq!(stop_tools.len(), 0, "Default stop_when_tools_called should be empty");

    let per_tool = ls["per_tool_max_iteration"].as_array()
        .expect("per_tool_max_iteration should be an array");
    assert_eq!(per_tool.len(), 0, "Default per_tool_max_iteration should be empty");
}

// ============================================================================
// Test 3: max_iteration limit enforcement
// ============================================================================

/// Test that max_iteration stops the loop after N iterations
///
/// This test uses a low max_iteration (1) to verify the limit is enforced.
/// With max_iteration=1, the loop should stop after the first iteration
/// regardless of whether there are more tool calls.
#[tokio::test]
async fn test_loop_settings_max_iteration_limit() {
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

    // Set loop_settings with max_iteration = 1
    let loop_settings = json!({
        "max_iteration": 1,
        "stop_when_no_tool_calling": true
    });

    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", Some(loop_settings)).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    eprintln!("\n=== Test: test_loop_settings_max_iteration_limit ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}'", i, event.event);
    }

    // Verify single complete event
    assert_single_complete(&events);

    // Check that we have at most 1 iteration of tool calls
    // With max_iteration=1, we should have at most 1 mcpToolComplete
    let tool_complete_count = count_events_by_type(&events, "mcpToolComplete");
    eprintln!("Tool complete events: {}", tool_complete_count);

    // The key assertion: max_iteration=1 means at most 1 full iteration
    // (This may be 0 if LLM doesn't call tool, or 1 if it does)
    assert!(
        tool_complete_count <= 1,
        "With max_iteration=1, should have at most 1 tool execution, got {}",
        tool_complete_count
    );
}

// ============================================================================
// Test 4: max_iteration = 0 means unlimited
// ============================================================================

/// Test that max_iteration = 0 means unlimited (uses natural completion)
#[tokio::test]
async fn test_loop_settings_max_iteration_zero_unlimited() {
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

    // Set loop_settings with max_iteration = 0 (unlimited)
    let loop_settings = json!({
        "max_iteration": 0,
        "stop_when_no_tool_calling": true
    });

    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", Some(loop_settings)).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    eprintln!("\n=== Test: test_loop_settings_max_iteration_zero_unlimited ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}'", i, event.event);
    }

    // Verify single complete event
    assert_single_complete(&events);

    // Verify the complete event has finish_reason "stop" (natural completion)
    // and NOT "max_iterations" (which would indicate hitting a limit)
    let complete_event = events.iter()
        .find(|e| e.event == "complete")
        .expect("Should have complete event");

    let finish_reason = complete_event.data.get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    eprintln!("Finish reason: {}", finish_reason);

    // With max_iteration=0 and stop_when_no_tool_calling=true,
    // the loop should complete naturally when LLM stops calling tools
    assert!(
        finish_reason == "stop" || finish_reason == "end_turn",
        "With max_iteration=0, should complete naturally with finish_reason 'stop' or 'end_turn', got '{}'",
        finish_reason
    );
}

// ============================================================================
// Test 5: stop_when_tools_called enforcement
// ============================================================================

/// Test that stop_when_tools_called stops when specific tool is called
///
/// This test sets the "fetch" tool as a stop tool, then triggers it.
/// The loop should complete immediately after the tool executes.
#[tokio::test]
async fn test_loop_settings_stop_when_tools_called() {
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

    // Set loop_settings with stop_when_tools_called = ["fetch"]
    // This should stop the loop immediately when the fetch tool is called
    let loop_settings = json!({
        "max_iteration": 10,  // High limit to not interfere
        "stop_when_no_tool_calling": true,
        "stop_when_tools_called": [
            { "server_id": mcp_server_id.to_string(), "tool_name": "fetch" }
        ]
    });

    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", Some(loop_settings)).await;

    // Send message that triggers the fetch tool
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    eprintln!("\n=== Test: test_loop_settings_stop_when_tools_called ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}'", i, event.event);
    }

    // Verify single complete event
    assert_single_complete(&events);

    // If the tool was called, verify it completed
    let tool_start_count = count_events_by_type(&events, "mcpToolStart");
    let tool_complete_count = count_events_by_type(&events, "mcpToolComplete");

    eprintln!("Tool start events: {}", tool_start_count);
    eprintln!("Tool complete events: {}", tool_complete_count);

    // With stop_when_tools_called set to "fetch", once the fetch tool is called
    // and executed, the loop should complete.
    // We should have exactly 1 tool execution (if the LLM called the tool)
    if tool_start_count > 0 {
        assert_eq!(
            tool_complete_count, 1,
            "With stop_when_tools_called, should have exactly 1 tool completion after the stop tool is called"
        );
    }
}

// ============================================================================
// Test 6: stop_when_no_tool_calling = true (default behavior)
// ============================================================================

/// Test that stop_when_no_tool_calling = true stops when LLM doesn't call tools
#[tokio::test]
async fn test_loop_settings_stop_when_no_tool_calling_true() {
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

    // Set loop_settings with stop_when_no_tool_calling = true (default)
    let loop_settings = json!({
        "max_iteration": 10,
        "stop_when_no_tool_calling": true
    });

    set_mcp_settings_with_loop(&server, &user.token, conversation_id, "auto_approve", Some(loop_settings)).await;

    // Send message that triggers tool use
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
    )
    .await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    // Parse SSE events
    let events = super::helpers::parse_sse_events(response).await;

    eprintln!("\n=== Test: test_loop_settings_stop_when_no_tool_calling_true ===");
    eprintln!("Total events received: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        eprintln!("Event {}: type='{}'", i, event.event);
    }

    // Verify single complete event
    assert_single_complete(&events);

    // Verify the complete event has a valid finish_reason
    let complete_event = events.iter()
        .find(|e| e.event == "complete")
        .expect("Should have complete event");

    let finish_reason = complete_event.data.get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    eprintln!("Finish reason: {}", finish_reason);

    // With stop_when_no_tool_calling=true, the loop should complete
    // when the LLM stops calling tools (finish_reason="stop")
    // or when max_iterations is reached
    assert!(
        finish_reason == "stop" || finish_reason == "end_turn" || finish_reason == "max_iterations",
        "Should have valid finish_reason, got '{}'",
        finish_reason
    );
}
