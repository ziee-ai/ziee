//! TEST-10 (fix-duplicate-tool-result): the approval row is CLAIMED — deleted —
//! before the approved tool executes, so execution is exactly-once.
//!
//! `execute_approved_tools_sync` used to delete the approval row AFTER running the
//! tool, with the error swallowed ("This may cause duplicate execution attempts"),
//! and four separate error arms each carried their own duplicate delete. A failed
//! DELETE therefore left the row `status='approved'`, and the next
//! `before_llm_call` re-found it via `get_approved_tools_for_branch`, RE-RAN the
//! tool and appended a SECOND `tool_result` row for the same `tool_use_id`. The row
//! is now claimed once, up front, before any execution path can run — the single
//! delete point in the loop.
//!
//! Drives the REAL path (scriptable OpenAI stub → stream finalize → MCP extension →
//! approval → resume) against an in-process HTTP MCP mock, mirroring
//! `mcp_approval_loop_test.rs`. The decisive assertions are that the mock received
//! EXACTLY ONE `tools/call` and that exactly ONE `tool_result` row exists for the
//! tool_use_id — a re-execution would show up as two of each.

use serde_json::json;
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, parse_uuid, send_body_and_collect_events};
use crate::common::oai_capture_stub::{StubChat, StubPlan, StubToolCall};
use crate::common::stub_chat::register_stub_model;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

async fn register_http_mcp(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "approval-claim mock",
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

async fn db_pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// A mock advertising a single `echo` tool. Deliberately queues MANY `tools/call`
/// responses: if the claim regressed and the tool were re-executed, the mock must
/// be able to answer the second call so the test fails on the COUNT assertion
/// (a real duplicate) rather than on a starved mock (an ambiguous error).
async fn start_echo_mock() -> MockMcpServer {
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method(
            "tools/list",
            MockResponse::JsonOk(json!({
                "tools": [ {
                    "name": "echo",
                    "description": "Echo the input",
                    "inputSchema": { "type": "object", "properties": {}, "additionalProperties": true }
                } ]
            })),
        );
    }
    for _ in 0..20 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({
                "content": [ { "type": "text", "text": "echo-ok" } ],
                "isError": false,
            })),
        );
    }
    mock
}

fn send_body(
    content: &str,
    model_id: Uuid,
    branch_id: Uuid,
    mcp_id: Uuid,
    approvals: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut body = json!({
        "content": content,
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": mcp_id, "tools": [] } ] },
    });
    if let Some(a) = approvals {
        body["tool_approvals"] = a;
    }
    body
}

#[tokio::test]
async fn approved_tool_is_claimed_and_executes_exactly_once() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "claim_once", &["*"]).await;

    let mock = start_echo_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, "claim_once_mock", &mock.base_url()).await;

    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "toolu_claim_once".to_string(),
            name: "echo".to_string(),
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

    // Manual-approve so the tool pauses rather than auto-running.
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "manual_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    // --- Send 1: pauses for approval; nothing has executed yet. ---
    let events1 = send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        send_body("run echo", model_id, branch_id, mcp_id, None),
        &[],
    )
    .await;
    assert!(
        events1.iter().any(|e| e.event == "mcpApprovalRequired"),
        "send 1 should request approval; events={:?}",
        events1.iter().map(|e| &e.event).collect::<Vec<_>>()
    );
    assert_eq!(
        mock.count_for("tools/call"),
        0,
        "nothing may execute before approval"
    );

    let pool = db_pool(&server).await;
    let tool_use_id: String = sqlx::query_scalar(
        "SELECT tool_use_id FROM tool_use_approvals WHERE branch_id = $1 \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(branch_id)
    .fetch_one(&pool)
    .await
    .expect("a pending approval row");

    // --- Send 2: approve → the tool runs, and the row is claimed. ---
    let approvals = json!([ { "tool_use_id": tool_use_id, "decision": "approved" } ]);
    send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        send_body("run echo", model_id, branch_id, mcp_id, Some(approvals)),
        &[],
    )
    .await;

    // The claim consumed the row: a later before_llm_call can no longer re-find it.
    let still_approved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tool_use_approvals WHERE branch_id = $1 AND tool_use_id = $2",
    )
    .bind(branch_id)
    .bind(&tool_use_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still_approved, 0,
        "the approval row must be claimed (deleted) — a surviving row is what let the \
         tool be re-executed and a second tool_result row appended"
    );

    // Exactly-once execution.
    assert_eq!(
        mock.count_for("tools/call"),
        1,
        "the approved tool must execute exactly once"
    );

    // …and exactly ONE persisted tool_result for it. Two rows here is the duplicate
    // this feature exists to prevent (history reconstruction would dedup it, but the
    // stored history would be wrong and get_tool_result recall reads the LAST row).
    let result_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM message_contents mc \
         JOIN messages m ON m.id = mc.message_id \
         WHERE m.branch_id = $1 AND mc.content_type = 'tool_result' \
           AND mc.content->>'tool_use_id' = $2",
    )
    .bind(branch_id)
    .bind(&tool_use_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        result_rows, 1,
        "exactly one tool_result row must be persisted for this tool_use_id"
    );
}
