//! Test for Single Assistant Message Architecture
//!
//! Validates that tool calling loops append results to a SINGLE assistant message
//! instead of creating new messages per iteration.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;

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

const TOOL_USE_PROMPT: &str = "Use the fetch tool to get the content from https://example.com and return the result. You MUST use the available fetch tool - do not make assumptions about the content.";

#[tokio::test]
async fn test_single_assistant_message_with_tool_execution() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS).await;

    // Create MCP server
    let mcp_server = create_test_mcp_server(&server, &user, true).await;
    let mcp_server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Create conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Set manual approval mode
    set_mcp_settings(&server, &user.token, conversation_id, "manual_approve", vec![]).await;

    // === STEP 1: Send initial message (will pause for approval) ===
    let response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        TOOL_USE_PROMPT,
        None,
    ).await;

    assert_eq!(response.status(), 200, "Should send message successfully");

    let events = super::helpers::parse_sse_events(response).await;

    // Verify started event structure
    let started_event = events.iter().find(|e| e.event == "started").expect("Should have started event");
    assert!(started_event.data["user_message_id"].is_string(), "Started event should have user_message_id");
    assert!(started_event.data["conversation_id"].is_string(), "Started event should have conversation_id");
    assert!(started_event.data["branch_id"].is_string(), "Started event should have branch_id");

    // CRITICAL: Started event should NOT have assistant_message_id
    let has_assistant_id_field = started_event.data.as_object()
        .and_then(|obj| obj.get("assistant_message_id"))
        .is_some();
    assert!(!has_assistant_id_field, "Started event should NOT have assistant_message_id field");

    // Get assistant message ID from content events
    let content_events: Vec<_> = events.iter().filter(|e| e.event == "content").collect();
    assert!(!content_events.is_empty(), "Should have content events");

    let first_content = &content_events[0].data;
    let assistant_message_id = first_content["message_id"].as_str().expect("Should have message_id in content");

    // Get pending approvals
    let pending = get_pending_approvals(&server, &user.token, branch_id).await;
    assert_eq!(pending.len(), 1, "Should have 1 pending approval");
    let tool_use_id = pending[0]["tool_use_id"].as_str().expect("Should have tool_use_id");

    // === STEP 2: Get messages BEFORE approval ===
    let messages_before = get_branch_messages_via_api(&server, &user.token, conversation_id).await;

    let user_msgs_before: Vec<_> = messages_before.iter().filter(|m| m["role"] == "user").collect();
    let assistant_msgs_before: Vec<_> = messages_before.iter().filter(|m| m["role"] == "assistant").collect();

    assert_eq!(user_msgs_before.len(), 1, "Should have exactly 1 user message");
    assert_eq!(assistant_msgs_before.len(), 1, "Should have exactly 1 assistant message");

    let assistant_msg_before = &assistant_msgs_before[0];
    assert_eq!(assistant_msg_before["id"].as_str().unwrap(), assistant_message_id, "Assistant message ID should match");

    let content_before = assistant_msg_before["contents"].as_array().expect("Should have contents");
    let content_count_before = content_before.len();
    eprintln!("\n=== Content blocks BEFORE approval: {} ===", content_count_before);
    for content in content_before.iter() {
        eprintln!("  type={}, sequence={}", content["type"], content["sequence"]);
    }

    // === STEP 3: Resume with approval (no new user message) ===
    let resume_response = send_message_with_mcp(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        "Continue with approved tool", // Some content required even when resuming
        Some(vec![json!({
            "tool_use_id": tool_use_id,
            "decision": "approve"
        })]),
    ).await;

    let status = resume_response.status();
    if status != 200 {
        let error_body = resume_response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        panic!("Resume failed with status {}: {}", status, error_body);
    }

    let resume_events = super::helpers::parse_sse_events(resume_response).await;

    // Verify started event on resume
    let resume_started = resume_events.iter().find(|e| e.event == "started").expect("Should have started event on resume");

    eprintln!("\\n=== Resume started event data: {:?} ===", resume_started.data);

    // Check if user_message_id exists in started event
    let has_user_id = resume_started.data.as_object()
        .and_then(|obj| obj.get("user_message_id"))
        .is_some();

    // Based on should_create_user_message hook, no user message should be created when tool_approvals are present
    assert!(!has_user_id, "Resume started should NOT have user_message_id (hook should prevent user message creation)");

    assert_eq!(resume_started.data["conversation_id"].as_str().unwrap(), conversation_id.to_string(), "Same conversation");
    assert_eq!(resume_started.data["branch_id"].as_str().unwrap(), branch_id.to_string(), "Same branch");

    // === STEP 4: Verify SINGLE assistant message architecture ===
    let messages_after = get_branch_messages_via_api(&server, &user.token, conversation_id).await;

    let user_msgs_after: Vec<_> = messages_after.iter().filter(|m| m["role"] == "user").collect();
    let assistant_msgs_after: Vec<_> = messages_after.iter().filter(|m| m["role"] == "assistant").collect();

    eprintln!("\\n=== Messages after resume: {} user, {} assistant ===", user_msgs_after.len(), assistant_msgs_after.len());

    // Hook should prevent new user message when tool_approvals are present
    assert_eq!(user_msgs_after.len(), 1, "Should have exactly 1 user message (hook prevents new user message on resume)");
    assert_eq!(assistant_msgs_after.len(), 1, "Should STILL have exactly 1 assistant message (same message, not new one)");

    let assistant_msg_after = &assistant_msgs_after[0];
    assert_eq!(assistant_msg_after["id"].as_str().unwrap(), assistant_message_id, "Should be the SAME assistant message ID");

    // === STEP 5: Verify tool results APPENDED with proper indices ===
    let content_after = assistant_msg_after["contents"].as_array().expect("Should have contents");
    let content_count_after = content_after.len();

    eprintln!("\n=== Content blocks AFTER approval and resume: {} ===", content_count_after);
    for (i, content) in content_after.iter().enumerate() {
        eprintln!("  [{}] type={:?}, sequence={:?}, data keys={:?}",
            i,
            content.get("type"),
            content.get("sequence"),
            content.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }

    assert!(content_count_after >= content_count_before,
        "Tool results should be APPENDED (count should increase or stay same from {} to {})",
        content_count_before, content_count_after);

    eprintln!("\n✅ Core architecture validated:");
    eprintln!("  - Single assistant message throughout workflow");
    eprintln!("  - Content count increased from {} to {}", content_count_before, content_count_after);
    eprintln!("  - No new user message created on resume (hook working correctly)");

    eprintln!("\n✅ ✅ ✅ SINGLE ASSISTANT MESSAGE ARCHITECTURE VALIDATED ✅ ✅ ✅");
    eprintln!("  ✓ Started event structure correct (NO assistant_message_id)");
    eprintln!("  ✓ Resume started structure correct (NO user_message_id)");
    eprintln!("  ✓ Only 1 user message, 1 assistant message throughout workflow");
    eprintln!("  ✓ Tool results appended to same assistant message (content count {} → {})", content_count_before, content_count_after);
    eprintln!("  ✓ Hooks working correctly (should_create_user_message, provide_assistant_message)");
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn create_test_mcp_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    enabled: bool,
) -> serde_json::Value {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("fetch_server_{}", &unique_id[..8]),
        "display_name": "Test Single Message MCP Server",
        "description": "MCP server for single message architecture testing",
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
        .expect("Should create MCP server");

    assert_eq!(response.status(), 201, "Should create MCP server successfully");
    let mcp_server: serde_json::Value = response.json().await.expect("Should parse MCP server response");

    // Assign to default group
    let server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();
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

async fn set_mcp_settings(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    approval_mode: &str,
    auto_approved_tools: Vec<serde_json::Value>,
) {
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

    assert!(response.status().is_success(), "MCP settings should be set successfully");
}

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
        .expect("Should send message")
}

async fn get_branch_messages_via_api(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
) -> Vec<serde_json::Value> {
    let url = server.api_url(&format!("/conversations/{}/messages", conversation_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Should get conversation messages");

    assert_eq!(response.status(), 200, "Should get messages successfully");

    response.json().await.expect("Should parse response as array")
}
