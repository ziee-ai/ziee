//! MCP Elicitation Integration Tests (chat-extension level)
//!
//! Exercises the full elicitation roundtrip through the chat extension:
//!
//!   user message
//!     → assistant invokes the MCP tool
//!     → tool emits `elicitation/create` on the SSE stream
//!     → backend forwards `mcpElicitationRequired` to the chat client
//!     → test POSTs to `/api/mcp/elicitation/{id}/respond`
//!     → backend delivers the response to the registry
//!     → backend POSTs the response back to the MCP server
//!     → MCP server emits the tool result
//!     → assistant generates the final message
//!
//! Uses [`MockElicitationServer`] as the upstream MCP server (in-process
//! axum) and a real LLM provider to make the tool-calling decision.

use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

use crate::common::{test_helpers, TestServer};
use crate::mcp::fixtures::mock_elicitation_server::{ElicitationScript, MockElicitationServer};

const MCP_ELICIT_PERMISSIONS: &[&str] = &[
    "conversations::create",
    "conversations::read",
    "conversations::edit",
    "messages::create",
    "messages::read",
    "mcp_servers::read",
    "llm_models::read",
    "llm_models::create",
    "llm_providers::read",
    "llm_providers::create",
    "llm_providers::edit",
    "mcp_servers_admin::create",
    "mcp_servers_admin::read",
];

// ─── Setup helpers ─────────────────────────────────────────────────────────

async fn create_elicit_mcp_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    mock: &MockElicitationServer,
) -> Value {
    let payload = json!({
        "name": format!("mock_elicit_{}", &Uuid::new_v4().to_string()[..8]),
        "display_name": "Mock Elicitation Server",
        "description": "In-process MCP server for elicitation chat tests",
        "enabled": true,
        "transport_type": "http",
        "url": mock.base_url(),
        "supports_sampling": false,
        "usage_mode": "auto",
        "timeout_seconds": 120
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("create system MCP server");
    assert_eq!(response.status(), 201, "Should create elicitation MCP server");

    let mcp: Value = response.json().await.unwrap();
    let server_id = Uuid::parse_str(mcp["id"].as_str().unwrap()).unwrap();

    // Assign to the default group so the test user can access it
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&server.database_url)
        .await
        .expect("connect test DB");
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
    .expect("assign to default group");
    pool.close().await;

    mcp
}

async fn set_mcp_settings_auto_approve(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
) {
    let response = reqwest::Client::new()
        .put(&server.api_url(&format!(
            "/conversations/{}/mcp-settings",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": []
        }))
        .send()
        .await
        .expect("set MCP settings");
    assert!(response.status().is_success(), "auto-approve must apply");
}

/// Send a chat message and return the raw streaming Response (do NOT consume
/// the body — the caller will stream it incrementally to handle elicitation).
async fn send_streaming_message(
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
    let resp = reqwest::Client::new()
        .post(&server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("send chat message");
    assert_eq!(resp.status(), 200, "stream request must succeed");
    resp
}

#[derive(Debug, Clone)]
struct ChatSseEvent {
    event: String,
    data: Value,
}

/// Stream the chat SSE response and intercept `mcpElicitationRequired`
/// events. When one fires, POST the given response back to the elicitation
/// endpoint. Returns all events observed plus the captured elicitation_ids.
///
/// Bounded by a generous timeout so a stuck stream fails the test fast.
async fn stream_until_complete(
    server: &TestServer,
    user_token: &str,
    response: reqwest::Response,
    answer: ElicitAnswer,
    overall_timeout: Duration,
) -> (Vec<ChatSseEvent>, Vec<String>) {
    let mut events: Vec<ChatSseEvent> = Vec::new();
    let mut elicitation_ids: Vec<String> = Vec::new();

    let task = async {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => panic!("stream chunk error: {}", e),
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Split on \n\n event separator
            while let Some(idx) = buffer.find("\n\n") {
                let raw = buffer[..idx].to_string();
                buffer.drain(..idx + 2);

                let mut name = String::from("message");
                let mut data_str = String::new();
                for line in raw.lines() {
                    if let Some(rest) = line.strip_prefix("event: ") {
                        name = rest.trim().to_string();
                    }
                    if let Some(rest) = line.strip_prefix("data: ") {
                        // SSE allows multi-line data fields; concatenate.
                        if !data_str.is_empty() {
                            data_str.push('\n');
                        }
                        data_str.push_str(rest);
                    }
                }
                if data_str.is_empty() {
                    continue;
                }
                let data: Value = match serde_json::from_str(&data_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                events.push(ChatSseEvent {
                    event: name.clone(),
                    data: data.clone(),
                });

                if name == "mcpElicitationRequired" {
                    let eid = data["elicitation_id"]
                        .as_str()
                        .expect("elicitation_id in event")
                        .to_string();
                    elicitation_ids.push(eid.clone());

                    // POST the user's response.
                    let url = server.api_url(&format!("/mcp/elicitation/{}/respond", eid));
                    let body = answer.to_body();
                    let resp = reqwest::Client::new()
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", user_token))
                        .json(&body)
                        .send()
                        .await
                        .expect("respond POST");
                    assert_eq!(
                        resp.status(),
                        200,
                        "elicitation respond must succeed for id={}", eid
                    );
                }

                if name == "complete" {
                    return;
                }
            }
        }
    };

    match tokio::time::timeout(overall_timeout, task).await {
        Ok(()) => (events, elicitation_ids),
        Err(_) => panic!(
            "chat stream timed out after {:?} (events so far: {})",
            overall_timeout,
            events.len()
        ),
    }
}

#[derive(Debug, Clone)]
enum ElicitAnswer {
    Accept(Value),
    Decline,
    Cancel,
}

impl ElicitAnswer {
    fn to_body(&self) -> Value {
        match self {
            Self::Accept(content) => json!({"action": "accept", "content": content}),
            Self::Decline => json!({"action": "decline"}),
            Self::Cancel => json!({"action": "cancel"}),
        }
    }
}

/// End-to-end scaffolding shared by every test in this file.
async fn run_elicit_scenario(
    server: &TestServer,
    mock: MockElicitationServer,
    prompt: &str,
    answer: ElicitAnswer,
    overall_timeout: Duration,
) -> (
    Vec<ChatSseEvent>,
    Vec<String>,
    MockElicitationServer,
    Uuid,
) {
    let user = test_helpers::create_user_with_permissions(
        server,
        "elicit_user",
        MCP_ELICIT_PERMISSIONS,
    )
    .await;
    let user_id = user.user_id.clone();

    let mcp = create_elicit_mcp_server(server, &user, &mock).await;
    let mcp_server_id = super::helpers::parse_uuid(&mcp["id"]);

    let conversation = super::helpers::create_conversation(server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(server, &user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    set_mcp_settings_auto_approve(server, &user.token, conversation_id).await;

    let resp = send_streaming_message(
        server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_server_id,
        prompt,
    )
    .await;

    let (events, eids) =
        stream_until_complete(server, &user.token, resp, answer, overall_timeout).await;
    (events, eids, mock, conversation_id)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

/// Test A: full happy-path roundtrip with accept.
///
/// The LLM calls the elicitation tool, the test responds with accept+content,
/// and the mock confirms it received the accept body. The chat stream
/// completes without errors.
#[tokio::test]
async fn test_elicitation_chat_accept_full_flow() {
    let server = TestServer::start().await;

    let mock = MockElicitationServer::start_with_script(ElicitationScript {
        tool_name: "request_user_confirmation".to_string(),
        tool_description:
            "Request explicit user confirmation before performing a destructive action. \
             You MUST call this tool whenever the user asks you to confirm anything."
                .to_string(),
        tool_input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "What is being confirmed" }
            },
            "required": ["action"]
        }),
        message: "Are you sure you want to delete the production database?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": { "approve": { "type": "boolean" } },
            "required": ["approve"]
        }),
        tool_result_content: vec![json!({
            "type": "text",
            "text": "Confirmation handled. Deletion was approved by the user."
        })],
        elicitation_response_timeout: Duration::from_secs(60),
    })
    .await;

    let (events, eids, mock, _conv_id) = run_elicit_scenario(
        &server,
        mock,
        "Use the request_user_confirmation tool to confirm that I want to delete the production database.",
        ElicitAnswer::Accept(json!({"approve": true})),
        Duration::from_secs(180),
    )
    .await;

    assert_eq!(eids.len(), 1, "exactly 1 elicitation should have surfaced");

    // mcpToolComplete should appear with no error
    let tool_complete = events
        .iter()
        .find(|e| e.event == "mcpToolComplete")
        .expect("expected mcpToolComplete event");
    assert_eq!(
        tool_complete.data["is_error"].as_bool(),
        Some(false),
        "tool must complete without error"
    );

    // Mock should have received exactly 1 elicitation response with action=accept
    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1, "mock should receive 1 accept response");
    assert_eq!(responses[0]["result"]["action"], "accept");
    assert_eq!(responses[0]["result"]["content"]["approve"], true);
}

/// Test B: decline path — the chat stream must still complete, the mock
/// must receive a decline (with no `content` field), and the tool must
/// finish without error.
#[tokio::test]
async fn test_elicitation_chat_decline_full_flow() {
    let server = TestServer::start().await;

    let mock = MockElicitationServer::start_with_script(ElicitationScript {
        tool_name: "request_user_confirmation".to_string(),
        tool_description:
            "Ask the user to approve an action. You MUST call this when the user asks \
             you to confirm something."
                .to_string(),
        tool_input_schema: json!({
            "type": "object",
            "properties": { "action": { "type": "string" } },
            "required": ["action"]
        }),
        message: "Approve the operation?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": { "approve": { "type": "boolean" } }
        }),
        tool_result_content: vec![json!({
            "type": "text",
            "text": "User declined. Operation aborted."
        })],
        elicitation_response_timeout: Duration::from_secs(60),
    })
    .await;

    let (_events, eids, mock, _conv_id) = run_elicit_scenario(
        &server,
        mock,
        "Use the request_user_confirmation tool to confirm the deployment.",
        ElicitAnswer::Decline,
        Duration::from_secs(180),
    )
    .await;

    assert_eq!(eids.len(), 1);

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["action"], "decline");
    assert!(
        responses[0]["result"].get("content").is_none(),
        "decline must omit `content`; got: {}",
        responses[0]["result"]
    );
}

/// Test C: cancel path — same as decline but with action=cancel.
#[tokio::test]
async fn test_elicitation_chat_cancel_full_flow() {
    let server = TestServer::start().await;

    let mock = MockElicitationServer::start_with_script(ElicitationScript {
        tool_name: "request_user_confirmation".to_string(),
        tool_description:
            "Ask the user to confirm. You MUST call this when the user mentions confirming."
                .to_string(),
        tool_input_schema: json!({
            "type": "object",
            "properties": { "action": { "type": "string" } },
            "required": ["action"]
        }),
        message: "Confirm?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": { "ok": { "type": "boolean" } }
        }),
        tool_result_content: vec![json!({
            "type": "text",
            "text": "User cancelled."
        })],
        elicitation_response_timeout: Duration::from_secs(60),
    })
    .await;

    let (_events, eids, mock, _conv_id) = run_elicit_scenario(
        &server,
        mock,
        "Use the request_user_confirmation tool to confirm the deployment.",
        ElicitAnswer::Cancel,
        Duration::from_secs(180),
    )
    .await;

    assert_eq!(eids.len(), 1);

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["action"], "cancel");
}
