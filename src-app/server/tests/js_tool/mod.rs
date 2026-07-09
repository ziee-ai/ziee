//! Integration tests for the `js_tool` / `run_js` built-in.
//!
//! These exercise the REAL production path: a (stub) tool-capable model emits a
//! `run_js` tool call → mcp.rs intercepts it → the js_tool executor runs the
//! script on the embedded QuickJS runtime → `ziee.tools.*` host functions
//! re-enter the MCP dispatcher against a `MockMcpServer` → the sub-tool call is
//! recorded in `mcp_tool_calls` with `source='script'`. Only the external
//! boundaries (the LLM provider = stub-engine, the MCP server = MockMcpServer)
//! are mocked; everything in between is the shipping code.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::stub_chat::StubChat;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

// ── helpers ────────────────────────────────────────────────────────────────

/// A user with all permissions (provider/model/mcp management + js_tool::use).
async fn power_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["*"]).await
}

async fn create_conversation(server: &TestServer, user: &TestUser, model_id: &str) -> (String, String) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("create conv");
    assert_eq!(resp.status(), 201, "create conv: {}", resp.text().await.unwrap_or_default());
    let v: Value = resp.json().await.unwrap();
    (
        v["id"].as_str().unwrap().to_string(),
        v["active_branch_id"].as_str().unwrap().to_string(),
    )
}

/// Send a message and drain the chat stream to the terminal frame, returning the
/// assembled assistant text.
async fn send_collect(
    server: &TestServer,
    user: &TestUser,
    conversation_id: &str,
    branch_id: &str,
    model_id: &str,
    content: &str,
) -> String {
    use crate::common::chat_stream_probe::ChatStreamProbe;
    let conv = Uuid::parse_str(conversation_id).unwrap();
    let mut probe = ChatStreamProbe::open(server, &user.token).await;
    probe.subscribe(Some(conv)).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conversation_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": content, "model_id": model_id, "branch_id": branch_id }))
        .send()
        .await
        .expect("send message");
    assert!(resp.status().is_success(), "send: {}", resp.text().await.unwrap_or_default());

    let frames = probe.collect_until_terminal(conv, std::time::Duration::from_secs(45)).await;
    ChatStreamProbe::assemble_text(&frames)
}

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Poll for recorded `source='script'` tool-call rows (the insert is
/// fire-and-forget, so it may land a beat after the turn completes).
async fn wait_for_script_rows(pool: &sqlx::PgPool, user_id: Uuid, want: i64) -> Vec<(String, String)> {
    for _ in 0..50 {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT tool_name, source FROM mcp_tool_calls
             WHERE user_id = $1 AND source = 'script' ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .unwrap();
        if rows.len() as i64 >= want {
            return rows;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Vec::new()
}

/// POST a JSON-RPC body to the run_js loopback endpoint.
async fn jsonrpc(server: &TestServer, token: &str, body: Value) -> (reqwest::StatusCode, Value) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/run-js/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    (status, v)
}

// ── tests ──────────────────────────────────────────────────────────────────

/// TEST-18: the built-in run_js mcp_servers row is registered as an editable
/// built-in (is_built_in/is_system/http/loopback url).
#[tokio::test]
async fn test_run_js_server_registered() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"run_js.ziee.internal");
    // Boot registration is a spawned upsert; poll briefly.
    let mut row: Option<(bool, bool, String, String)> = None;
    for _ in 0..50 {
        row = sqlx::query_as(
            "SELECT is_built_in, is_system, transport_type, url FROM mcp_servers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if row.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let (is_built_in, is_system, transport, url) = row.expect("run_js server row registered");
    assert!(is_built_in, "run_js is_built_in");
    assert!(is_system, "run_js is_system");
    assert_eq!(transport, "http");
    assert!(url.contains("/api/run-js/mcp"), "loopback url: {url}");
}

/// TEST-19: the loopback JSON-RPC handler answers initialize + tools/list (the
/// run_js descriptor) and refuses tools/call ("invoke in chat context").
#[tokio::test]
async fn test_run_js_jsonrpc_handler() {
    let server = TestServer::start().await;
    let user = power_user(&server, "js_handler").await;

    let (st, init) = jsonrpc(&server, &user.token, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})).await;
    assert_eq!(st, 200);
    assert_eq!(init["result"]["serverInfo"]["name"], "run_js");

    let (st, list) = jsonrpc(&server, &user.token, json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})).await;
    assert_eq!(st, 200);
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"run_js"), "tools/list advertises run_js: {names:?}");

    let (_st, call) = jsonrpc(&server, &user.token, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run_js","arguments":{"script":"return 1"}}})).await;
    assert!(call["error"].is_object(), "tools/call over loopback must error (chat-context only): {call}");
}

/// TEST-25: the handler is gated on `js_tool::use` — unauthenticated → 401.
#[tokio::test]
async fn test_run_js_handler_requires_auth() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(server.api_url("/run-js/mcp"))
        .header("content-type", "application/json")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "unauthenticated run_js handler must be 401");
}

/// TEST-9: a run_js script calling `ziee.tools.echo` dispatches through the REAL
/// MCP dispatcher (MockMcpServer) and the sub-tool call is recorded with
/// `source='script'`.
#[tokio::test]
async fn test_run_js_dispatch_records_source_script() {
    let server = TestServer::start().await;
    let user = power_user(&server, "js_dispatch").await;
    let stub = StubChat::start().await;
    let model_id = crate::common::stub_chat::register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;
    let (conv, branch) = create_conversation(&server, &user, &model_id).await;

    // run_js calls the always-available `get_tool_result` built-in (bypasses
    // approval), which re-enters the dispatcher and is recorded source='script'.
    let text = send_collect(&server, &user, &conv, &branch, &model_id, "STUB_PLAN=run_js_echo call it").await;
    assert!(text.contains("dispatched"), "run_js dispatched a sub-tool; final text: {text}");

    let uid = Uuid::parse_str(&user.user_id).unwrap();
    let rows = wait_for_script_rows(&pool(&server).await, uid, 1).await;
    assert!(
        rows.iter().any(|(tool, src)| tool == "get_tool_result" && src == "script"),
        "a sub-tool call recorded with source=script: {rows:?}"
    );
}
/// TEST-15: a run_js script that LOOPS the tool over items records N sub-tool
/// calls with source='script', while the model's context only receives the
/// single run_js summary (the intermediate results stay in the script).
#[tokio::test]
async fn test_run_js_loop_records_all_sub_calls() {
    let server = TestServer::start().await;
    let user = power_user(&server, "js_loop").await;
    let stub = StubChat::start().await;
    let model_id = crate::common::stub_chat::register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;
    let (conv, branch) = create_conversation(&server, &user, &model_id).await;

    // The script loops the sub-tool 3x; only the SUMMARY value reaches the model
    // (the 3 intermediate results stay inside the script), yet all 3 sub-calls
    // are recorded source='script'.
    let text = send_collect(&server, &user, &conv, &branch, &model_id, "STUB_PLAN=run_js_loop over items").await;
    assert!(text.contains("calls"), "the run_js summary should reach the model: {text}");

    let uid = Uuid::parse_str(&user.user_id).unwrap();
    let rows = wait_for_script_rows(&pool(&server).await, uid, 3).await;
    assert_eq!(rows.len(), 3, "three sub-calls recorded with source=script: {rows:?}");
}
/// TEST-27 + TEST-28: a script that returns a value surfaces it to the model;
/// a script that throws surfaces an error with the (preamble-corrected) line.
#[tokio::test]
async fn test_run_js_value_and_error() {
    let server = TestServer::start().await;
    let user = power_user(&server, "js_valerr").await;
    let stub = StubChat::start().await;
    let model_id = crate::common::stub_chat::register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, true, None).await;

    // Value: `return 6*42` → the model's continuation echoes "42".
    let (conv, branch) = create_conversation(&server, &user, &model_id).await;
    let text = send_collect(&server, &user, &conv, &branch, &model_id, "STUB_PLAN=run_js_value go").await;
    assert!(text.contains("42"), "run_js return value 42 reaches the model: {text}");

    // Error: `throw new Error('boom from script')` on user line 2 → error digest
    // with the user line, surfaced to the model.
    let (conv2, branch2) = create_conversation(&server, &user, &model_id).await;
    let text2 = send_collect(&server, &user, &conv2, &branch2, &model_id, "STUB_PLAN=run_js_error go").await;
    assert!(text2.contains("boom from script"), "run_js error reaches the model: {text2}");
    // The digest carries a line number (exact user-line mapping is pinned by the
    // runtime unit test `test_error_line_maps_to_user_line`).
    assert!(text2.contains("run_js error (line"), "error digest carries a line: {text2}");
}
