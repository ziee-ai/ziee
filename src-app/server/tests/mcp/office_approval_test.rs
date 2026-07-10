//! Office `run_office_js` approval-path integration tests.
//!
//! These exercise the office-mode-gated-approval feature end-to-end through the
//! REAL chat → MCP → approval loop, using [`MockOfficeServer`] registered under the
//! deterministic `office_bridge_mcp_server_id()`. The desktop `office_bridge` daemon
//! is not involved (and cannot be — it is desktop-only); the mock stands in for the
//! pane, because the server-side decision keys only on the server id + tool + `mode`.
//!
//! - **TEST-17** — real LLM: a READ task → the model declares `mode:"read"` → the
//!   tool AUTO-RUNS even under manual-approve (no pending approval); a WRITE task →
//!   the model declares `mode:"write"` → a pending approval is created and the tool
//!   is WITHHELD (the mock records no execution).
//! - **TEST-15** — DENY: a WRITE task pauses for approval; resuming with a `denied`
//!   decision means the tool NEVER executes (the mock records zero `run_office_js`
//!   calls).
//!
//! Both drive a real model, so they soft-skip when no provider key is configured.
//! Run against the coder.ziee OpenAI-compatible endpoint:
//!   `OPENAI_API_KEY=sk-litellm-dummy OPENAI_BASE_URL=http://127.0.0.1:4000 \
//!      cargo test --test integration_tests -- --test-threads=1 mcp::office_approval_test`

use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;
use crate::mcp::mock_office_server::MockOfficeServer;

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

/// True when at least one LLM provider key is configured (the real-LLM tiers
/// need one; on a keyless box these tests soft-skip rather than fail).
fn llm_configured() -> bool {
    ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY", "GROQ_API_KEY"]
        .iter()
        .any(|k| std::env::var(k).is_ok())
}

/// Register the mock office server as a system MCP server, then FORCE its DB id to
/// the deterministic `office_bridge_mcp_server_id()` so the approval decision sees
/// it as the office bridge. Returns that id.
async fn register_mock_office_server(
    server: &TestServer,
    user: &test_helpers::TestUser,
    mock: &MockOfficeServer,
) -> Uuid {
    let payload = json!({
        "name": format!("mock_office_{}", &Uuid::new_v4().to_string()[..8]),
        "display_name": "Mock Office Bridge",
        "description": "In-process mock office_bridge for approval-path tests",
        "enabled": true,
        "transport_type": "http",
        "url": mock.url(),
        "usage_mode": "auto",
        "timeout_seconds": 120
    });

    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create mock office system server");
    assert_eq!(
        resp.status(),
        201,
        "Should create mock office system server: {:?}",
        resp.text().await
    );
    let created: Value = resp.json().await.expect("parse create response");
    let random_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let office_id = ziee::chat_extension::office_bridge_mcp_server_id();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&server.database_url)
        .await
        .expect("connect test db");

    // Swap the primary key to the deterministic office id BEFORE any FK row
    // references it, then assign the (office-id) row to the default group.
    sqlx::query!(
        "UPDATE mcp_servers SET id = $1 WHERE id = $2",
        office_id,
        random_id
    )
    .execute(&pool)
    .await
    .expect("swap mock office server id → deterministic office_bridge id");

    let default_group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("get default group");

    sqlx::query!(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at)
         VALUES ($1, $2, NOW())",
        default_group.id,
        office_id
    )
    .execute(&pool)
    .await
    .expect("assign mock office server to default group");

    pool.close().await;
    office_id
}

/// Set the conversation's approval mode, pre-approving the `list_open_documents`
/// DISCOVERY tool for the office server. This isolates the test to the
/// `run_office_js` decision: the model reliably calls `list_open_documents` first
/// to resolve a `doc_full_name`, and that native tool (no `mode`) gates like any
/// tool under manual-approve — pre-approving it lets the turn REACH `run_office_js`,
/// whose auto-run-vs-prompt is then decided ONLY by the office read/write bypass.
async fn set_mcp_settings_discovery_preapproved(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    mode: &str,
    office_id: Uuid,
) {
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}/mcp-settings", conversation_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "approval_mode": mode,
            "auto_approved_tools": [{"server_id": office_id, "tools": ["list_open_documents"]}]
        }))
        .send()
        .await
        .expect("set mcp settings");
    assert!(resp.status().is_success(), "set mcp settings: {:?}", resp.text().await);
}

fn run_office_js_pending<'a>(pending: &'a [Value]) -> Option<&'a Value> {
    pending
        .iter()
        .find(|a| a["tool_name"].as_str() == Some("run_office_js"))
}

async fn get_pending_approvals(server: &TestServer, token: &str, branch_id: Uuid) -> Vec<Value> {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/branches/{}/pending-approvals", branch_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("get pending approvals");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse approvals");
    body["approvals"].as_array().cloned().unwrap_or_default()
}

fn message_body(
    content: &str,
    branch_id: Uuid,
    model_id: Uuid,
    mcp_server_id: Uuid,
    tool_approvals: Option<Vec<Value>>,
) -> Value {
    let mut body = json!({
        "content": content,
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [{"server_id": mcp_server_id, "tools": []}]
        }
    });
    if let Some(approvals) = tool_approvals {
        body["tool_approvals"] = json!(approvals);
    }
    body
}

async fn send(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    mcp_server_id: Uuid,
    content: &str,
    tool_approvals: Option<Vec<Value>>,
) -> Vec<crate::chat::helpers::SSEEvent> {
    let body = message_body(content, branch_id, model_id, mcp_server_id, tool_approvals);
    crate::chat::helpers::send_body_and_collect_events(server, token, conversation_id, body, &[]).await
}

const READ_PROMPT: &str = "Use the run_office_js tool to READ the value of cell A1 of the open \
    workbook and report it. The target document's doc_full_name is \"Book1.xlsx\". Write a script \
    that only loads and returns the value — do not change anything.";

const WRITE_PROMPT: &str = "Use the run_office_js tool to SET the value of cell A1 of the open \
    workbook to the text hello. The target document's doc_full_name is \"Book1.xlsx\".";

/// TEST-17 — real-LLM end-to-end: a READ task auto-runs (no approval) and a WRITE
/// task creates a pending approval and is withheld, both under manual-approve mode,
/// driven purely by the model's `mode` declaration on the SHIPPED schema.
#[tokio::test]
async fn test17_read_auto_runs_write_requires_approval() {
    if !llm_configured() {
        eprintln!("SKIP test17: no LLM provider key configured (source tests/.env.test)");
        return;
    }
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS).await;
    let mock = MockOfficeServer::start().await;
    let office_id = register_mock_office_server(&server, &user, &mock).await;

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // ---- READ phase: manual-approve, but a run_office_js READ must AUTO-RUN ----
    // (list_open_documents is pre-approved, so the ONLY thing that lets run_office_js
    // execute without a prompt is the office read-bypass — a normal server would gate it.)
    let read_convo = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let read_cid = crate::chat::helpers::parse_uuid(&read_convo["id"]);
    let read_bid = crate::chat::helpers::parse_uuid(&read_convo["active_branch_id"]);
    set_mcp_settings_discovery_preapproved(&server, &user.token, read_cid, "manual_approve", office_id).await;

    let read_events = send(
        &server, &user.token, read_cid, read_bid, model_id, office_id, READ_PROMPT, None,
    )
    .await;

    let read_pending = get_pending_approvals(&server, &user.token, read_bid).await;
    let read_calls = mock.calls().await;
    let read_run_calls: Vec<_> = read_calls.iter().filter(|c| c.tool_name == "run_office_js").collect();

    assert!(
        !read_run_calls.is_empty(),
        "READ: run_office_js should have EXECUTED (auto-run via the office read-bypass) — \
         recorded calls: {:?}, events: {:?}",
        read_calls,
        read_events.iter().map(|e| &e.event).collect::<Vec<_>>()
    );
    assert!(
        read_run_calls.iter().all(|c| c.mode.as_deref() == Some("read")),
        "READ: the model should declare mode=read; recorded: {:?}",
        read_run_calls
    );
    assert!(
        run_office_js_pending(&read_pending).is_none(),
        "READ: a run_office_js read must NOT create a pending approval; got {:?}",
        read_pending
    );

    // ---- WRITE phase: a run_office_js WRITE must PAUSE for approval and NOT execute ----
    let write_convo = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let write_cid = crate::chat::helpers::parse_uuid(&write_convo["id"]);
    let write_bid = crate::chat::helpers::parse_uuid(&write_convo["active_branch_id"]);
    set_mcp_settings_discovery_preapproved(&server, &user.token, write_cid, "manual_approve", office_id).await;

    let run_calls_before_write = mock.run_office_js_call_count().await;

    let write_events = send(
        &server, &user.token, write_cid, write_bid, model_id, office_id, WRITE_PROMPT, None,
    )
    .await;

    let write_pending = get_pending_approvals(&server, &user.token, write_bid).await;
    assert!(
        run_office_js_pending(&write_pending).is_some(),
        "WRITE: a run_office_js write must create a pending approval; pending={:?}, events={:?}",
        write_pending,
        write_events.iter().map(|e| &e.event).collect::<Vec<_>>()
    );
    assert_eq!(
        mock.run_office_js_call_count().await,
        run_calls_before_write,
        "WRITE: the tool must NOT execute while approval is pending"
    );
}

/// TEST-15 — DENY: a WRITE task pauses for approval; resuming with `denied` means
/// the tool never executes (the mock records zero run_office_js calls).
#[tokio::test]
async fn test15_denied_write_never_executes() {
    if !llm_configured() {
        eprintln!("SKIP test15: no LLM provider key configured (source tests/.env.test)");
        return;
    }
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", MCP_TEST_PERMISSIONS).await;
    let mock = MockOfficeServer::start().await;
    let office_id = register_mock_office_server(&server, &user, &mock).await;

    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    let convo = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let cid = crate::chat::helpers::parse_uuid(&convo["id"]);
    let bid = crate::chat::helpers::parse_uuid(&convo["active_branch_id"]);
    set_mcp_settings_discovery_preapproved(&server, &user.token, cid, "manual_approve", office_id).await;

    // Write task → the run_office_js write must pause for approval (list_open_documents
    // is pre-approved, so the pending approval we resolve is the run_office_js one).
    let _e1 = send(&server, &user.token, cid, bid, model_id, office_id, WRITE_PROMPT, None).await;
    let pending = get_pending_approvals(&server, &user.token, bid).await;
    let office_approval = run_office_js_pending(&pending)
        .expect("write should create a pending approval FOR run_office_js (non-vacuous)");
    let tool_use_id = office_approval["tool_use_id"].as_str().unwrap().to_string();
    assert_eq!(
        mock.run_office_js_call_count().await,
        0,
        "tool must not have executed while pending"
    );

    // Resume with DENY.
    let deny = json!({"tool_use_id": tool_use_id, "decision": "denied"});
    let events = send(
        &server, &user.token, cid, bid, model_id, office_id, WRITE_PROMPT, Some(vec![deny]),
    )
    .await;

    assert_eq!(
        mock.run_office_js_call_count().await,
        0,
        "DENY: run_office_js must NEVER execute after a denied approval; events: {:?}",
        events.iter().map(|e| &e.event).collect::<Vec<_>>()
    );
    // The denied approval must be resolved, not still hanging.
    let still_pending = get_pending_approvals(&server, &user.token, bid).await;
    assert!(
        still_pending
            .iter()
            .all(|a| a["tool_use_id"].as_str() != Some(tool_use_id.as_str())),
        "DENY: the denied approval should be resolved, not still pending: {:?}",
        still_pending
    );
}
