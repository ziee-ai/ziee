//! Regression: a conversation whose tool returns `audience:["user"]` must still
//! get an auto-generated title.
//!
//! Reported as "BioGnosia conversations show *Untitled Conversation*" — a
//! DIFFERENT failure from the raw-first-message bug in `title_test.rs`, with a
//! different cause.
//!
//! BioGnosia's `query_rag` annotates its result `audience: ["user"]`, meaning the
//! tool result IS the final answer and the LLM round-trip is skipped. The MCP
//! extension (order 30) therefore returns `ExtensionAction::CompleteWithContent`
//! — and `ExtensionRegistry::call_after_llm_call` used to RETURN on the first
//! such action, so every extension ordered after MCP was silently skipped. The
//! title extension is order 80, so it never ran on ANY turn of such a
//! conversation and the title stayed NULL forever. (Confirmed on the live
//! deployment: a conversation with two completed `query_rag` turns, still
//! untitled.)
//!
//! Two things make this pass now: the registry runs later extensions for their
//! side effects instead of short-circuiting, and the title extension accepts a
//! `tool_result` as the turn's user-visible output (there is no text block on
//! this path — that is the whole point of `audience:["user"]`).
//!
//! Drives the REAL path: scriptable OpenAI stub → stream finalize → MCP
//! extension → audience detection → registry → title extension.

use serde_json::json;
use uuid::Uuid;

use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, get_conversation, parse_uuid, send_body_and_collect_events};
use crate::common::oai_capture_stub::{StubChat, StubPlan, StubToolCall};
use crate::common::stub_chat::{STUB_TITLE, register_stub_model};
use crate::common::TestServer;

async fn register_http_mcp(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "audience-user mock",
            "transport_type": "http",
            "url": url,
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(status, 201, "register mock server: {status}: {body}");
    let row: serde_json::Value = serde_json::from_str(&body).unwrap();
    Uuid::parse_str(row["id"].as_str().unwrap()).unwrap()
}

/// A mock whose single tool answers with `annotations.audience: ["user"]` — the
/// exact shape BioGnosia's `query_rag` returns.
async fn start_audience_user_mock() -> MockMcpServer {
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method(
            "tools/list",
            MockResponse::JsonOk(json!({
                "tools": [ {
                    "name": "query_rag",
                    "description": "Query the knowledge base",
                    "inputSchema": { "type": "object", "properties": {}, "additionalProperties": true }
                } ]
            })),
        );
    }
    for _ in 0..20 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({
                "content": [ {
                    "type": "text",
                    "text": "TP53 is the most frequently mutated gene in human cancer.",
                    "annotations": { "audience": ["user"] }
                } ],
                "isError": false,
            })),
        );
    }
    mock
}

#[tokio::test]
async fn an_audience_user_tool_turn_still_gets_a_title() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "title_audience",
        &["*"],
    )
    .await;

    let mock = start_audience_user_mock().await;
    let mcp_id =
        register_http_mcp(&server, &user.token, "title_audience_mock", &mock.base_url()).await;

    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "toolu_audience_title".to_string(),
            name: "query_rag".to_string(),
            arguments: "{}".to_string(),
        }],
        ..Default::default()
    };
    let stub = StubChat::start(plan).await;
    let model_id_s = register_stub_model(
        &server,
        &user.token,
        &user.user_id,
        &stub.base_url(),
        true,
        None,
    )
    .await;
    let model_id = Uuid::parse_str(&model_id_s).unwrap();

    let conversation = create_conversation(&server, &user.token, None, None).await;
    let conversation_id = parse_uuid(&conversation["id"]);
    let branch_id = parse_uuid(&conversation["active_branch_id"]);
    assert!(
        conversation["title"].is_null(),
        "must start untitled so the extension is what sets the title"
    );

    // Auto-approve so the tool runs immediately — this test is about what happens
    // AFTER an `audience:["user"]` result comes back, not about the approval gate.
    // NOTE: `disabled` is NOT "no approval prompt", it turns MCP off entirely for
    // the conversation.
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conversation_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "auto_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    let question = "What does the knowledge base say about TP53 mutations?";
    let events = send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        json!({
            "content": question,
            "model_id": model_id,
            "branch_id": branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
        }),
        &[],
    )
    .await;

    assert_eq!(
        mock.count_for("tools/call"),
        1,
        "the audience-user tool must actually have run; tools/list={} initialize={} events={:?} stub_requests={}",
        mock.count_for("tools/list"),
        mock.count_for("initialize"),
        events.iter().map(|e| &e.event).collect::<Vec<_>>(),
        stub.requests().len(),
    );

    let conv = get_conversation(&server, &user.token, conversation_id).await;
    let title = conv["title"].as_str();
    assert_eq!(
        title,
        Some(STUB_TITLE),
        "an audience-user turn must still be titled — this is the \
         'Untitled Conversation' regression"
    );
    assert_ne!(
        title,
        Some(question),
        "and it must not be the raw first user message either"
    );
}
