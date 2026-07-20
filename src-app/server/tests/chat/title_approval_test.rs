//! Regression: a conversation must be titled on its FIRST turn under
//! `manual_approve` — not only after the user sends a second message.
//!
//! A third distinct titling failure, after the raw-first-message bug
//! (`title_test.rs`) and the registry short-circuit (`title_audience_test.rs`).
//! Both of those are fixed; this one survived them, and it is the one that shows
//! on the production default config (every deployment conversation uses
//! `manual_approve`).
//!
//! ## Mechanism
//!
//! `call_after_llm_call` — where the title extension (order 80) runs — has
//! exactly ONE call site: `DeltaAccumulator::finalize`, which only runs after a
//! provider stream is consumed. On the approval-RESUME send, MCP's
//! `before_llm_call` executes the approved tool itself and returns
//! `BeforeLlmAction::CompleteWithContent` (the `audience:["user"]` result IS the
//! answer, so there is nothing to ask the LLM). Streaming appends that text and
//! `break`s straight out of the loop — no accumulator, no `finalize`, no
//! `after_llm_call`. The title extension was never reached AT ALL on that turn.
//!
//! Under `auto_approve` the same conversation titles correctly, because the tool
//! runs inside a normal LLM turn that does reach `finalize`. That asymmetry is
//! exactly what made this look like an approval bug rather than a titling bug.
//!
//! The fix is the `after_llm_skipped` hook: the two `BeforeLlmAction` break arms
//! run a turn-end extension pass. Re-calling `call_after_llm_call` there would
//! have re-entered MCP's own `after_llm_call` and risked executing approved tools
//! early, so the hook is deliberately separate.
//!
//! Drives the REAL path: scriptable OpenAI stub → MCP approval gate → pending
//! approval over REST → resume with `tool_approvals` → `before_llm_call`
//! executes the tool → `CompleteWithContent` → turn-end hook → title extension.

use serde_json::json;
use uuid::Uuid;

use crate::chat::helpers::{
    create_conversation, get_conversation, parse_uuid, send_body_and_collect_events,
};
use crate::common::TestServer;
use crate::common::oai_capture_stub::{StubChat, StubPlan, StubToolCall};
use crate::common::stub_chat::{STUB_TITLE, register_stub_model};
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};

const ANSWER: &str = "TP53 is the most frequently mutated gene in human cancer.";

async fn register_http_mcp(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "manual-approve mock",
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
/// exact shape BioGnosia's `query_rag` returns, and the shape that makes the
/// resume skip the LLM entirely.
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
                    "text": ANSWER,
                    "annotations": { "audience": ["user"] }
                } ],
                "isError": false,
            })),
        );
    }
    mock
}

async fn pending_approvals(
    server: &TestServer,
    token: &str,
    branch_id: Uuid,
) -> Vec<serde_json::Value> {
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/branches/{branch_id}/pending-approvals")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get pending approvals");
    assert_eq!(res.status(), 200, "pending-approvals should return 200");
    let body: serde_json::Value = res.json().await.expect("parse pending approvals");
    body["approvals"]
        .as_array()
        .expect("approvals should be an array")
        .clone()
}

struct Fixture {
    server: TestServer,
    token: String,
    conversation_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    mcp_id: Uuid,
    mock: MockMcpServer,
    #[allow(dead_code)]
    stub: StubChat,
}

/// Stand up a `manual_approve` conversation whose only tool answers with
/// `audience:["user"]`, and drive turn 1 up to the approval gate.
async fn arrange_manual_approve_turn_one(label: &str) -> Fixture {
    let server = TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, label, &["*"]).await;

    let mock = start_audience_user_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, label, &mock.base_url()).await;

    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "toolu_manual_approve_title".to_string(),
            name: "query_rag".to_string(),
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
    assert!(
        conversation["title"].is_null(),
        "must start untitled so the extension is what sets the title"
    );

    // THE POINT OF THIS TEST: manual_approve, the production default.
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conversation_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "manual_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    Fixture {
        server,
        token: user.token,
        conversation_id,
        branch_id,
        model_id,
        mcp_id,
        mock,
        stub,
    }
}

/// Turn 1: the model asks for the tool, MCP parks it for approval.
async fn send_turn_one(f: &Fixture, question: &str) {
    send_body_and_collect_events(
        &f.server,
        &f.token,
        f.conversation_id,
        json!({
            "content": question,
            "model_id": f.model_id,
            "branch_id": f.branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": f.mcp_id, "tools": [] } ] },
        }),
        &[],
    )
    .await;
}

/// TEST-1 + TEST-6: the headline regression.
#[tokio::test]
async fn manual_approve_titles_on_the_first_turn() {
    let f = arrange_manual_approve_turn_one("title_manual_approve").await;
    let question = "What does the knowledge base say about TP53 mutations?";

    send_turn_one(&f, question).await;

    // Turn 1 parked at the gate: nothing executed, still untitled.
    let approvals = pending_approvals(&f.server, &f.token, f.branch_id).await;
    assert_eq!(
        approvals.len(),
        1,
        "manual_approve must park the tool call for approval; got {approvals:?}"
    );
    assert_eq!(
        f.mock.count_for("tools/call"),
        0,
        "nothing may execute before the user approves"
    );
    let conv = get_conversation(&f.server, &f.token, f.conversation_id).await;
    assert!(
        conv["title"].is_null(),
        "still untitled while awaiting approval — the assistant has produced no answer yet"
    );

    let tool_use_id = approvals[0]["tool_use_id"].as_str().unwrap().to_string();

    // The resume. There is no separate approve endpoint: the decision rides a
    // fresh POST /messages carrying `tool_approvals`.
    let events = send_body_and_collect_events(
        &f.server,
        &f.token,
        f.conversation_id,
        json!({
            "content": "",
            "model_id": f.model_id,
            "branch_id": f.branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": f.mcp_id, "tools": [] } ] },
            "tool_approvals": [ { "tool_use_id": tool_use_id, "decision": "approved" } ],
        }),
        &[],
    )
    .await;

    assert_eq!(
        f.mock.count_for("tools/call"),
        1,
        "the approved tool must have run on the resume; events={:?}",
        events.iter().map(|e| &e.event).collect::<Vec<_>>()
    );

    // THE ASSERTION. Before the fix this was still null here, and only became
    // non-null once the user sent a SECOND message.
    let conv = get_conversation(&f.server, &f.token, f.conversation_id).await;
    let title = conv["title"].as_str();
    assert_eq!(
        title,
        Some(STUB_TITLE),
        "a manual_approve turn must be titled on the FIRST exchange, with no \
         second user message — this is the reported 'Untitled Conversation' bug"
    );
    assert_ne!(
        title,
        Some(question),
        "and it must not be the raw first user message either"
    );

    // NOT asserted here: that `titleUpdated` arrives BEFORE the terminal frame.
    //
    // It is tempting — the hook does enqueue the event before the terminal chunk
    // is sent — but the driver forwards the chunk stream and the extension
    // channel through ONE `tokio::select!`, which picks arbitrarily among ready
    // branches. So the two can be published in either order, and asserting the
    // order produced a genuinely flaky test (observed failing under
    // `--test-threads=4` while the title itself was correctly persisted).
    //
    // That ordering is not needed in production: the client's per-conversation
    // SSE connection is long-lived and keeps receiving after `complete`, and the
    // turn additionally publishes a `Conversation/Update` sync event that makes
    // every other surface refetch. The DELIVERY that matters — the title being
    // persisted on turn 1 — is asserted above and is deterministic.
    let _ = &events;
}

/// TEST-7: the all-denied path (`BeforeLlmAction::Complete`) must be a safe
/// no-op — no title, no panic. The turn produced no answer, so there is nothing
/// to title, and the new hook must not invent one.
#[tokio::test]
async fn a_denied_approval_generates_no_title() {
    let f = arrange_manual_approve_turn_one("title_manual_deny").await;

    send_turn_one(&f, "What does the knowledge base say about TP53 mutations?").await;

    let approvals = pending_approvals(&f.server, &f.token, f.branch_id).await;
    assert_eq!(approvals.len(), 1, "expected one parked approval");
    let tool_use_id = approvals[0]["tool_use_id"].as_str().unwrap().to_string();

    send_body_and_collect_events(
        &f.server,
        &f.token,
        f.conversation_id,
        json!({
            "content": "",
            "model_id": f.model_id,
            "branch_id": f.branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": f.mcp_id, "tools": [] } ] },
            "tool_approvals": [ { "tool_use_id": tool_use_id, "decision": "denied" } ],
        }),
        &[],
    )
    .await;

    assert_eq!(
        f.mock.count_for("tools/call"),
        0,
        "a denied tool must never execute"
    );

    let conv = get_conversation(&f.server, &f.token, f.conversation_id).await;
    assert!(
        conv["title"].is_null(),
        "a turn that produced no answer must stay untitled, not get a \
         speculative title from the new turn-end hook; got {:?}",
        conv["title"]
    );
}
