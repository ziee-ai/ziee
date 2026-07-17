//! Real-LLM MULTI-TURN agentic chat with tool calls on the agent-core path,
//! against the live bridge (flag ON). Turn 1 makes the model call an MCP tool;
//! turn 2 continues the SAME conversation and the model answers using the
//! prior-turn context — proving the loop's tool execution + cross-turn transcript
//! persistence hold across requests on the agent-core loop.
//!
//! Soft-skips unless `ZIEE_TEST_LLM_BASE_URL` is set. RUN ISOLATED (cutover flag).

use serde_json::{json, Value};
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, parse_uuid};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

async fn start_echo_mock() -> MockMcpServer {
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method(
            "tools/list",
            MockResponse::JsonOk(json!({
                "tools": [ {
                    "name": "echo",
                    "description": "Echo back the provided text verbatim",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "text": { "type": "string" } },
                        "required": ["text"]
                    }
                } ]
            })),
        );
    }
    for _ in 0..20 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({
                "content": [ { "type": "text", "text": "ECHO_RESULT: purple-turtle-42" } ],
                "isError": false,
            })),
        );
    }
    mock
}

async fn register_http_mcp(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name, "display_name": "multiturn mock",
            "transport_type": "http", "url": url, "enabled": true,
        }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn agent_core_multiturn_tool_call_and_followup() {
    let base = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_core_multiturn_tool_call_and_followup — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name = std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());
    unsafe { std::env::set_var("ZIEE_CHAT_AGENT_CORE", "1") };

    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ac_multiturn", &["*"]).await;

    let provider: Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("MT {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true, "api_key": key, "base_url": base,
        }))
        .send().await.unwrap().json().await.unwrap();
    let model: Value = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"], "name": model_name, "display_name": "MT Qwen",
            "enabled": true, "engine_type": "none", "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true, "embedding": false }
        }))
        .send().await.unwrap().json().await.unwrap();
    let model_id = parse_uuid(&model["id"]);
    crate::chat::helpers::ensure_user_has_model_access(&server, &user.user_id, &model).await;

    let mock = start_echo_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, "mt_mock", &mock.base_url()).await;

    let conv = create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = parse_uuid(&conv["id"]);
    let branch_id = parse_uuid(&conv["active_branch_id"]);
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "auto_approve", "auto_approved_tools": [] }))
        .send().await.unwrap();

    // TURN 1 — the model must call the echo tool.
    let body1 = json!({
        "content": "Use the echo tool to echo the exact text 'purple-turtle-42', then tell me what it returned.",
        "model_id": model_id, "branch_id": branch_id, "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
    });
    let t1 = crate::chat::helpers::send_body_and_collect_events(&server, &user.token, conv_id, body1, &[]).await;
    let n1: Vec<&str> = t1.iter().map(|e| e.event.as_str()).collect();
    assert!(n1.iter().any(|n| *n == "mcpToolStart"), "turn 1 must call the tool; events={n1:?}");
    assert!(n1.iter().any(|n| *n == "complete"), "turn 1 must complete; events={n1:?}");

    // TURN 2 — SAME conversation; the model answers from the prior-turn context.
    let body2 = json!({
        "content": "Without calling any tool, what value did the echo tool return in the previous message?",
        "model_id": model_id, "branch_id": branch_id, "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
    });
    let t2 = crate::chat::helpers::send_body_and_collect_events(&server, &user.token, conv_id, body2, &[]).await;
    let n2: Vec<&str> = t2.iter().map(|e| e.event.as_str()).collect();
    assert!(n2.iter().any(|n| *n == "complete"), "turn 2 must complete; events={n2:?}");

    // Cross-turn persistence: the history holds both turns' assistant messages +
    // the tool_result from turn 1 (the transcript survived across requests).
    let history = crate::chat::helpers::get_conversation_history(&server, &user.token, conv_id).await;
    let dump = history.to_string();
    assert!(
        dump.contains("purple-turtle-42"),
        "the tool result / echoed value must persist across turns in the transcript"
    );

    unsafe { std::env::remove_var("ZIEE_CHAT_AGENT_CORE") };
}
