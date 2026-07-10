//! Regression tests for the gpt-oss/harmony code_sandbox approval-loop bug.
//!
//! A local gpt-oss/harmony model emits tool calls WITHOUT the `<server_id>__`
//! prefix ziee prepends (bare `execute_command` or empty-prefix `__query_rag`),
//! so the finalized tool_use had an empty server_id → the approval row was
//! stored with `server_id = NULL` → `execute_approved_tools_sync` silently
//! `continue`d without deleting the row → the agentic loop re-found it every
//! iteration until "Tool execution stopped: maximum iteration limit reached".
//!
//! These tests drive the REAL chat path (custom provider → OpenAIProvider →
//! stream finalize → MCP extension → approval → resume) with a scriptable
//! OpenAI stub emitting a BARE tool name, and an in-process HTTP MCP mock:
//!  - `mcp_approval_loop_bare_name_recovers_and_executes` — the bare name is
//!    recovered to the advertising server, and after approval the tool ACTUALLY
//!    executes (the mock receives a `tools/call`) instead of spinning.
//!  - `mcp_approval_loop_unresolvable_tool_errors_and_terminates` — an
//!    unresolvable bare name surfaces a clear error and deletes the approval row
//!    instead of looping to max_iteration.
//!
//! The decisive anti-loop assertion is `StubChat::request_count()`: the buggy
//! path re-called the LLM ~10× (one per wasted iteration); the fixed path calls
//! it ~twice (initial + resume), so a small count proves the spin is gone.

use serde_json::json;
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::chat::helpers::{create_conversation, parse_uuid, send_body_and_collect_events};
use crate::common::oai_capture_stub::{StubChat, StubPlan, StubToolCall};
use crate::common::stub_chat::register_stub_model;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

/// Register `url` as a user-owned HTTP MCP server; return its id.
async fn register_http_mcp(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "approval-loop mock",
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

/// Open a pool on the per-test DB.
async fn db_pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// The single tool-use approval row for `branch_id` (server_id may be NULL), or
/// None if there is none.
async fn latest_approval(
    pool: &sqlx::PgPool,
    branch_id: Uuid,
) -> Option<(String, Option<Uuid>, String)> {
    sqlx::query_as::<_, (String, Option<Uuid>, String)>(
        "SELECT tool_use_id, server_id, status FROM tool_use_approvals
         WHERE branch_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(branch_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Build the chat body used by both sends (initial + resume).
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

/// A mock that advertises a single tool `echo` and answers `tools/call`.
///
/// `on_method` responses are consumed FIFO and `tools/list` is called once per
/// connection/iteration, so queue plenty of each to keep `echo` advertised (and
/// `tools/call` answered) across both sends of a test.
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

/// True if any collected frame's data mentions the max-iteration cap.
fn mentions_max_iteration(frames: &[crate::chat::helpers::SSEEvent]) -> bool {
    frames
        .iter()
        .any(|f| f.data.to_string().contains("maximum iteration limit reached"))
}

// TEST-10 — bare tool name is recovered to its advertising server, the approval
// row carries that server_id (not NULL), and after approval the tool ACTUALLY
// executes (the mock receives a tools/call) without spinning to max_iteration.
#[tokio::test]
async fn mcp_approval_loop_bare_name_recovers_and_executes() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "loop_exec", &["*"]).await;

    let mock = start_echo_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, "loop_exec_mock", &mock.base_url()).await;

    // Stub emits a BARE tool name (no `<server_id>__` prefix), mimicking
    // gpt-oss/harmony, plus a non-empty id.
    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "tool_use".to_string(),
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

    // Manual-approve so the tool pauses for approval.
    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "manual_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    // --- Send 1: expect a pending approval carrying the RECOVERED server_id ---
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
    assert!(!mentions_max_iteration(&events1), "send 1 must not hit max_iteration");

    let pool = db_pool(&server).await;
    let (tool_use_id, server_id, status) =
        latest_approval(&pool, branch_id).await.expect("a pending approval row");
    assert_eq!(status, "pending");
    assert!(!tool_use_id.is_empty(), "tool_use_id must be non-empty");
    assert_eq!(
        server_id,
        Some(mcp_id),
        "bare tool name must be recovered to the advertising server (Fix A)"
    );

    // --- Send 2: approve → tool executes on the mock; no runaway loop ---
    let approvals = json!([ { "tool_use_id": tool_use_id, "decision": "approved" } ]);
    let events2 = send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        send_body("run echo", model_id, branch_id, mcp_id, Some(approvals)),
        &[],
    )
    .await;
    assert!(!mentions_max_iteration(&events2), "resume must not hit max_iteration");

    // The mock actually received a tools/call for echo — the approved tool ran.
    let calls = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .count();
    assert!(calls >= 1, "approved tool should have executed (tools/call on the mock)");

    // Fix B, end-to-end: the resume re-emitted a call with the SAME provider id
    // (`tool_use`); the `used_ids` DB-seed (which now finds the first, persisted
    // `tool_use` id) must mint a FRESH unique id for it, so the new pending
    // approval carries a minted `call_` id distinct from the first — and the bare
    // name is still recovered to the server.
    let (new_tuid, new_sid, _new_status) = latest_approval(&pool, branch_id)
        .await
        .expect("a new pending approval after resume");
    assert_ne!(
        new_tuid, tool_use_id,
        "the re-emitted duplicate provider id must be minted to a fresh unique id"
    );
    assert!(
        new_tuid.starts_with("call_"),
        "the minted id should be call_<uuid>, got {new_tuid}"
    );
    assert_eq!(
        new_sid,
        Some(mcp_id),
        "the re-emitted bare name is still recovered to the advertising server"
    );

    // Anti-loop: the buggy path re-called the LLM ~10× (one per wasted
    // iteration). The fixed path calls it ~twice (send 1 + resume).
    assert!(
        stub.request_count() <= 4,
        "LLM should not be re-called in a loop; got {} calls",
        stub.request_count()
    );
}

// TEST-8 — an unresolvable bare name (advertised by no server) surfaces a clear
// error and DELETES the approval row after approval, instead of spinning to
// max_iteration. Covers the `server_id == None` loud-error branch (ITEM-1).
#[tokio::test]
async fn mcp_approval_loop_unresolvable_tool_errors_and_terminates() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "loop_err", &["*"]).await;

    // The mock advertises `echo`, but the model will emit a DIFFERENT bare name
    // that maps to no server → unrecoverable → NULL server_id approval.
    let mock = start_echo_mock().await;
    let mcp_id = register_http_mcp(&server, &user.token, "loop_err_mock", &mock.base_url()).await;

    let plan = StubPlan {
        text: String::new(),
        tool_calls: vec![StubToolCall {
            id: "tool_use".to_string(),
            name: "ghost_tool".to_string(), // bare + not advertised
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

    reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "approval_mode": "manual_approve", "auto_approved_tools": [] }))
        .send()
        .await
        .unwrap();

    let events1 = send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        send_body("run ghost", model_id, branch_id, mcp_id, None),
        &[],
    )
    .await;
    assert!(
        events1.iter().any(|e| e.event == "mcpApprovalRequired"),
        "send 1 should request approval"
    );

    let pool = db_pool(&server).await;
    let (tool_use_id, server_id, _status) =
        latest_approval(&pool, branch_id).await.expect("a pending approval row");
    assert_eq!(
        server_id, None,
        "an unresolvable bare name yields a NULL-server_id approval"
    );

    // Approve → the null-server_id branch must surface an error AND delete the
    // row, terminating the turn instead of looping.
    let approvals = json!([ { "tool_use_id": tool_use_id, "decision": "approved" } ]);
    let events2 = send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        send_body("run ghost", model_id, branch_id, mcp_id, Some(approvals)),
        &[],
    )
    .await;
    assert!(!mentions_max_iteration(&events2), "resume must not hit max_iteration");

    // The original NULL-server_id approval row is gone (deleted, not re-looped).
    let remaining = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tool_use_approvals WHERE branch_id = $1 AND tool_use_id = $2 AND server_id IS NULL",
    )
    .bind(branch_id)
    .bind(&tool_use_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0, "the NULL-server_id approval must be deleted, not re-looped");

    assert!(
        stub.request_count() <= 4,
        "LLM should not be re-called in a loop; got {} calls",
        stub.request_count()
    );
}
