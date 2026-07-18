//! Real-LLM verification of the chat→agent-core TOOL path (ITEM-25) against the
//! live bridge. A tool-capable model, given an `echo` MCP tool + an auto-approve
//! conversation, must CALL the tool through the agent-core loop (ChatToolProvider),
//! emit the `mcpToolStart`/`mcpToolComplete` SSE lifecycle, and produce a final
//! answer — proving tool execution + streaming parity end-to-end on the flag.
//!
//! Soft-skips unless `ZIEE_TEST_LLM_BASE_URL` is set. Bridge coords from
//! `server/tests/.env.test`. RUN ISOLATED (sets a process-global flag).

use serde_json::{json, Value};
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, parse_uuid, send_body_and_collect_events};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

/// A mock advertising a single `echo` tool that answers `tools/call`.
async fn start_echo_mock() -> MockMcpServer {
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method(
            "tools/list",
            MockResponse::JsonOk(json!({
                "tools": [ {
                    "name": "echo",
                    "description": "Echo back the provided text",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "text": { "type": "string", "description": "text to echo" } },
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
                "content": [ { "type": "text", "text": "ECHO: hello world" } ],
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
            "name": name, "display_name": "ac tool mock",
            "transport_type": "http", "url": url, "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn agent_core_bridge_tool_execute_and_respond() {
    let base = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_core_bridge_tool_execute_and_respond — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name =
        std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());
    let _agent_core_flag = crate::common::AgentCoreFlag::on();

    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ac_bridge_tool", &["*"]).await;

    // A custom (OpenAI-compatible) provider pointing at the bridge.
    let provider: Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("Bridge {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true,
            "api_key": key, "base_url": base,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // A tool-capable model named for the bridge's served model.
    let model: Value = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"],
            "name": model_name,
            "display_name": "Bridge Qwen (tools)",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true, "embedding": false }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let model_id = parse_uuid(&model["id"]);
    crate::chat::helpers::ensure_user_has_model_access(&server, &user.user_id, &model).await;

    let mock = start_echo_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, "ac_tool_mock", &mock.base_url()).await;

    let conv = create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = parse_uuid(&conv["id"]);
    let branch_id = parse_uuid(&conv["active_branch_id"]);

    // Auto-approve so the tool executes inline (tests the execution path).
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "auto_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    let body = json!({
        "content": "Use the echo tool to echo the text 'hello world', then tell me exactly what it returned.",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
    });
    let events = send_body_and_collect_events(&server, &user.token, conv_id, body, &[]).await;
    let names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();

    assert!(
        events.iter().any(|e| e.event == "mcpToolStart"),
        "the model should call the echo tool (mcpToolStart) on the agent-core path; events={names:?}"
    );
    assert!(
        events.iter().any(|e| e.event == "mcpToolComplete"),
        "the tool call should complete (mcpToolComplete); events={names:?}"
    );
    assert!(
        events.iter().any(|e| e.event == "complete"),
        "the turn should end on a complete frame; events={names:?}"
    );
}
