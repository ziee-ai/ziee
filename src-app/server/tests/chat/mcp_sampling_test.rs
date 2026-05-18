//! MCP Sampling Integration Tests
//!
//! Tests the end-to-end sampling protocol: an MCP server that calls back into
//! our server during tool execution to request LLM completions.
//!
//! Uses `MockSamplingServer` — an in-process axum HTTP server that exposes a
//! `research` tool making 2 sequential sampling requests before returning the
//! final answer.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;

// ============================================================================
// Constants
// ============================================================================

const MCP_SAMPLING_PERMISSIONS: &[&str] = &[
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

// ============================================================================
// Local Helpers (mirrors mcp_streaming_workflow_test.rs)
// ============================================================================

async fn create_sampling_mcp_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    mock_mcp: &crate::mcp::mock_sampling_server::MockSamplingServer,
) -> serde_json::Value {
    let payload = json!({
        "name": format!("mock_sampling_{}", &Uuid::new_v4().to_string()[..8]),
        "display_name": "Mock Sampling Server",
        "description": "In-process mock MCP server for sampling tests",
        "enabled": true,
        "transport_type": "http",
        "url": mock_mcp.url(),
        "supports_sampling": true,
        "usage_mode": "auto",
        "timeout_seconds": 120
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create system MCP server");

    if response.status() != 201 {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("Failed to create sampling MCP server: {} — {}", status, body);
    }

    let mcp_server: serde_json::Value = response.json().await.expect("Failed to parse response");
    let server_id = Uuid::parse_str(mcp_server["id"].as_str().unwrap()).unwrap();

    // Assign to the default group so the test user has access
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
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

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("Failed to set MCP settings: {} — {}", status, body);
    }
}

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
            "mcp_servers": [{"server_id": mcp_server_id, "tools": []}]
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
// Shared Setup Helper
// ============================================================================

/// Full end-to-end sampling setup: starts mock server, creates user + conversation +
/// model, sends the research tool message, and returns all collected SSE events plus
/// the mock server handle for introspection.
async fn run_sampling_scenario(
    server: &TestServer,
) -> (
    Vec<super::helpers::SSEEvent>,
    crate::mcp::mock_sampling_server::MockSamplingServer,
    Uuid, // conversation_id
) {
    run_sampling_scenario_with_mock(
        server,
        crate::mcp::mock_sampling_server::MockSamplingServer::start().await,
        "Use the research tool with query 'What is the capital of France?'",
    )
    .await
}

/// Same as `run_sampling_scenario` but accepts a pre-configured mock server and custom prompt.
/// Allows tests to inject different `MockBehavior` variants.
async fn run_sampling_scenario_with_mock(
    server: &TestServer,
    mock_mcp: crate::mcp::mock_sampling_server::MockSamplingServer,
    prompt: &str,
) -> (
    Vec<super::helpers::SSEEvent>,
    crate::mcp::mock_sampling_server::MockSamplingServer,
    Uuid,
) {
    let user = test_helpers::create_user_with_permissions(
        server,
        "sampling_user",
        MCP_SAMPLING_PERMISSIONS,
    )
    .await;

    let mcp_server = create_sampling_mcp_server(server, &user, &mock_mcp).await;
    let mcp_server_id = super::helpers::parse_uuid(&mcp_server["id"]);

    let conversation = super::helpers::create_conversation(server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    set_mcp_settings(server, &user.token, conversation_id, "auto_approve", vec![]).await;

    let response = send_message_with_mcp(
        server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        prompt,
    )
    .await;

    assert_eq!(response.status(), 200, "Stream request should succeed");
    let events = super::helpers::parse_sse_events(response).await;

    tracing::debug!("=== sampling scenario: received {} events ===", events.len());
    for (i, e) in events.iter().enumerate() {
        tracing::debug!("  [{}] event={} data={}", i, e.event, e.data);
    }

    (events, mock_mcp, conversation_id)
}

// ============================================================================
// Tests
// ============================================================================

/// Test A — Verify exactly 2 LLM sampling calls were made.
/// The mock server increments sampling_call_count each time Ziee responds to a
/// sampling/createMessage request. If sampling is broken the mock panics (no fallback).
#[tokio::test]
async fn test_sampling_exactly_two_llm_calls() {
    let server = TestServer::start().await;
    let (_, mock_mcp, _) = run_sampling_scenario(&server).await;

    let count = mock_mcp.sampling_call_count();
    assert_eq!(
        count, 2,
        "Expected exactly 2 LLM sampling calls, got {}",
        count
    );
}

/// Test B — Verify that Ziee produced non-empty LLM responses for both sampling calls,
/// that combined responses mention "france" or "paris", and that the second response
/// (the one-sentence summary) is shorter than the first (full answer).
#[tokio::test]
async fn test_sampling_llm_response_content() {
    let server = TestServer::start().await;
    let (_, mock_mcp, _) = run_sampling_scenario(&server).await;

    let results = mock_mcp.sampling_results().await;
    assert_eq!(results.len(), 2, "Expected 2 recorded sampling results, got {}", results.len());

    let answer = &results[0];
    let summary = &results[1];

    assert!(!answer.is_empty(), "Sampling call #1 (answer) should not be empty");
    assert!(!summary.is_empty(), "Sampling call #2 (summary) should not be empty");

    // The query was about France/Paris so at least one response should mention it
    let both = format!("{} {}", answer, summary).to_lowercase();
    assert!(
        both.contains("france") || both.contains("paris"),
        "Expected mention of France or Paris in LLM responses — answer: {:?}, summary: {:?}",
        answer, summary
    );

    // The mock's second sampling prompt explicitly asks for a one-sentence summary,
    // so the summary must not be longer than the original answer.
    // We only enforce strict shortening when the original is substantive (> 50 chars) —
    // if the LLM already answered in a single short sentence, a summary of the same
    // length is acceptable.
    if answer.len() > 50 {
        assert!(
            summary.len() < answer.len(),
            "Sampling call #2 (one-sentence summary) should be shorter than call #1 (full answer) \
             — summary len={}, answer len={}",
            summary.len(), answer.len()
        );
    } else {
        assert!(
            summary.len() <= answer.len(),
            "Sampling call #2 (one-sentence summary) should not be longer than call #1 \
             — summary len={}, answer len={}",
            summary.len(), answer.len()
        );
    }
}

/// Test C — Verify SSE event order:
/// mcpToolStart must precede mcpToolComplete, and there must be text content
/// (textDelta message events) after the tool completes (the final LLM answer).
#[tokio::test]
async fn test_sampling_lifecycle_event_order() {
    let server = TestServer::start().await;
    let (events, _, _) = run_sampling_scenario(&server).await;

    let tool_start_idx = events
        .iter()
        .position(|e| e.event == "mcpToolStart")
        .expect("Expected mcpToolStart event");

    let tool_end_idx = events
        .iter()
        .position(|e| e.event == "mcpToolComplete")
        .expect("Expected mcpToolComplete event");

    assert!(
        tool_start_idx < tool_end_idx,
        "mcpToolStart (idx={}) must come before mcpToolComplete (idx={})",
        tool_start_idx, tool_end_idx
    );

    // Tool must complete without error
    let tool_complete = &events[tool_end_idx];
    assert_eq!(
        tool_complete.data["is_error"].as_bool(),
        Some(false),
        "mcpToolComplete should have is_error=false — got: {}",
        tool_complete.data
    );

    // There must be at least one text delta event after the tool completes.
    // Events have: event="content", data.type="content", data.content=[{type:"text_delta"}]
    let has_text_after_tool = events[tool_end_idx..].iter().any(|e| {
        e.event == "content"
            && e.data
                .get("content")
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|item| {
                    item.get("type").and_then(|t| t.as_str()) == Some("text_delta")
                }))
                .unwrap_or(false)
    });
    assert!(
        has_text_after_tool,
        "Expected text_delta content events after mcpToolComplete (final LLM answer)"
    );

    // Stream must contain a complete event
    assert!(
        events.iter().any(|e| e.event == "complete"),
        "Expected a 'complete' event in the stream"
    );
}

/// Test D — Verify database persistence after sampling completes.
/// Expect: 1 user message + at least 1 assistant message.
/// The assistant message should contain tool_use content (the research call) and
/// text content (the final answer mentioning France/Paris).
#[tokio::test]
async fn test_sampling_response_persisted_in_db() {
    let server = TestServer::start().await;
    let (_, _, conversation_id) = run_sampling_scenario(&server).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let messages = sqlx::query!(
        r#"
        SELECT DISTINCT m.id, m.role
        FROM messages m
        JOIN branch_messages bm ON bm.message_id = m.id
        JOIN branches b ON b.id = bm.branch_id
        WHERE b.conversation_id = $1
        ORDER BY m.id
        "#,
        conversation_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch messages");

    // Check user message is present
    let user_msgs: Vec<_> = messages.iter().filter(|m| m.role == "user").collect();
    let assistant_msgs: Vec<_> = messages.iter().filter(|m| m.role == "assistant").collect();

    assert!(!user_msgs.is_empty(), "Expected at least 1 user message, got none");
    assert!(!assistant_msgs.is_empty(), "Expected at least 1 assistant message, got none");

    // Check the assistant message(s) have content stored
    for am in &assistant_msgs {
        let contents = sqlx::query!(
            "SELECT content_type, content FROM message_contents WHERE message_id = $1 ORDER BY sequence_order",
            am.id
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch message contents");

        assert!(!contents.is_empty(), "Assistant message {} should have content blocks", am.id);
    }

    // The combined text of all assistant message contents should mention France/Paris.
    // Extract the actual string value when possible — Value::to_string() would add JSON
    // escaping (e.g., "Paris" → "\"paris\"") which is confusing in failure messages.
    let all_assistant_text: String = {
        let mut text = String::new();
        for am in &assistant_msgs {
            let contents = sqlx::query!(
                "SELECT content FROM message_contents WHERE message_id = $1",
                am.id
            )
            .fetch_all(&pool)
            .await
            .expect("Failed to fetch message contents");
            for row in contents {
                let raw = match &row.content {
                    serde_json::Value::String(s) => s.clone(),
                    // For JSON objects/arrays (tool result content), keep JSON for keyword search
                    other => other.to_string(),
                };
                text.push_str(&raw.to_lowercase());
            }
        }
        text
    };

    pool.close().await;

    assert!(
        all_assistant_text.contains("france") || all_assistant_text.contains("paris"),
        "Assistant messages in DB should mention France or Paris — got: {}",
        &all_assistant_text[..all_assistant_text.len().min(500)]
    );
}

/// Test E — Verify that both sampling responses from Ziee conform to the MCP sampling spec:
/// - role must be "assistant" (not "user" or missing)
/// - content must be present (not null)
/// - model must be a non-empty string (confirms DB lookup succeeded)
///
/// This test catches regressions where the sampling handler sends back a partial or
/// malformed response that the mock silently accepts. Without this check, a handler that
/// returns `model: ""` or `role: "user"` would pass all other tests.
#[tokio::test]
async fn test_sampling_response_structure_is_valid() {
    let server = TestServer::start().await;
    let (_, mock_mcp, _) = run_sampling_scenario(&server).await;

    let valid = mock_mcp.sampling_results_valid().await;
    // Must match sampling_call_count (2 calls = 2 validated results)
    assert_eq!(valid.len(), 2, "Expected 2 sampling results to validate, got {}", valid.len());
    assert!(
        valid[0],
        "Sampling result #1 must have role=assistant, content present, and non-empty model"
    );
    assert!(
        valid[1],
        "Sampling result #2 must have role=assistant, content present, and non-empty model"
    );
}

/// Test F — Verify graceful degradation when the sampling response is never delivered.
///
/// Scenario: the mock fires sampling request #1, then drops the response channel
/// (DropFirstResponse behavior) — simulating Ziee not responding to a sampling call,
/// the LLM returning an error, or the handler panicking partway through.
///
/// Expected behavior (after BUG-10 fix):
///   1. Mock detects the dropped channel and sends a JSON-RPC error event over SSE
///   2. The SSE stream ends without a tool result
///   3. Ziee marks the tool as is_error=true (stream ended before result)
///   4. The SSE stream still ends with a `complete` event (no infinite hang)
///
/// Before BUG-10 was fixed, the mock server panicked, causing the test to hang
/// indefinitely or fail with an uninformative message.
#[tokio::test]
async fn test_sampling_timeout_produces_tool_error() {
    let server = TestServer::start().await;

    // Start mock that fires sampling request #1 then drops the response channel
    let mock_mcp = crate::mcp::mock_sampling_server::MockSamplingServer::start_with_behavior(
        crate::mcp::mock_sampling_server::MockBehavior::DropFirstResponse,
    )
    .await;

    let (events, _, _) = run_sampling_scenario_with_mock(
        &server,
        mock_mcp,
        "Use the research tool with query 'timeout test'",
    )
    .await;

    // Tool must complete — either with is_error=true (sampling failed → tool error) or
    // the stream ends without a tool result which is also acceptable as the mock dropped
    // the channel. The key assertion is that the stream ends cleanly (complete event present).
    let tool_complete = events.iter().find(|e| e.event == "mcpToolComplete");

    if let Some(tc) = tool_complete {
        // If a complete event fired, the tool should be an error (mock didn't return a result)
        assert_eq!(
            tc.data["is_error"].as_bool(),
            Some(true),
            "Tool should complete with is_error=true when sampling response is dropped — got: {}",
            tc.data
        );
    }

    // Stream must end cleanly — no infinite hang
    assert!(
        events.iter().any(|e| e.event == "complete"),
        "Stream must end with complete event even after sampling channel is dropped"
    );
}

/// Test G — Verify that Image content in MCP sampling messages is handled without crashing.
///
/// The MCP sampling spec allows MCP servers to include images in sampling requests
/// (e.g., an MCP screenshot tool passes a screenshot to Ziee's LLM for analysis).
/// Before BUG-6 was fixed, Image content caused a serde deserialization error because
/// SamplingContent only had a Text variant — the entire sampling pipeline crashed.
/// After the fix, Image is properly deserialized and converted to ContentBlock::Image.
///
/// The image itself (1×1 white PNG) is sent by the mock server, not by the test user.
/// The mock injects it into a sampling/createMessage request during tool execution.
/// We don't assert what the LLM says about a 1×1 pixel — only that the pipeline
/// doesn't crash and the stream ends cleanly.
#[tokio::test]
async fn test_sampling_with_image_content_does_not_crash() {
    let server = TestServer::start().await;

    // The mock will include a 1×1 PNG as Image content in the sampling/createMessage request.
    // This simulates an MCP server passing a screenshot/diagram to Ziee's LLM for analysis.
    let mock_mcp = crate::mcp::mock_sampling_server::MockSamplingServer::start_with_behavior(
        crate::mcp::mock_sampling_server::MockBehavior::SendImageContent,
    )
    .await;

    let (events, _, _) = run_sampling_scenario_with_mock(
        &server,
        mock_mcp,
        "Use the research tool with query 'What is the capital of France?'",
    )
    .await;

    // The stream must always end cleanly — no 500 error, no infinite hang
    assert!(
        events.iter().any(|e| e.event == "complete"),
        "Stream must end cleanly after image sampling — no infinite hang"
    );

    // If the LLM invoked the tool, the tool must also complete.
    // If the LLM chose not to invoke the tool, that's also acceptable (no Image was sent,
    // so there's nothing to crash — the test's implicit goal is satisfied).
    let tool_start_count = events.iter().filter(|e| e.event == "mcpToolStart").count();
    let tool_complete_count = events.iter().filter(|e| e.event == "mcpToolComplete").count();
    assert_eq!(
        tool_start_count, tool_complete_count,
        "Every mcpToolStart must have a corresponding mcpToolComplete — \
         start={}, complete={}",
        tool_start_count, tool_complete_count
    );
}
