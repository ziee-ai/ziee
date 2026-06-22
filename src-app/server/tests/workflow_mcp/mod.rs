//! workflow_mcp — run a workflow FROM A CONVERSATION via the built-in
//! `POST /api/workflows/mcp` JSON-RPC server (the path the chat LLM uses to
//! invoke an installed workflow as ONE opaque tool).
//!
//! Unlike the REST `/run` path (always `invocation_source='manual'`), the
//! `tools/call` path stamps `invocation_source='conversation'` and scopes the
//! run to the originating conversation (resolved from the `x-conversation-id`
//! header). These tests prove that wiring end-to-end:
//!
//!   - `tools/call wf_<slug>` (+ `x-conversation-id`) spawns a run whose row has
//!     `invocation_source='conversation'` (direct SQL), the call returns the
//!     formatted `CallToolResult`, and the run completes;
//!   - the JSON-RPC auth gate: a caller WITHOUT `workflows::execute` → 403;
//!   - `initialize` / `tools/list` shape (the workflow surfaces as a `wf_<slug>`
//!     tool with the input schema derived from `workflow.inputs[]`).
//!
//! The workflow's sole `llm` step is short-circuited by a baked-in YAML `mock:`
//! (honored because the dev import sets `is_dev=true`, and the MCP path passes
//! an empty runtime mocks map → the runner falls back to `StepDef.mock`). So
//! this whole module needs NO real LLM key — the conversation only supplies the
//! model SNAPSHOT (a stub model), never a real token.

mod resources_test;
mod upsert_test;

use serde_json::{Value as Json, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};
use crate::workflow::{import_dev_workflow, poll_run};

/// A single-step `llm` workflow whose step is mock-short-circuited via a
/// baked-in YAML `mock:` (no real provider call). `inputs.topic` feeds the
/// derived MCP `inputSchema` (required).
const MCP_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "What to summarize"
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
    mock: "MCP_MOCK_SUMMARY: a canned summary, no tokens spent"
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: full
"#;

/// The tool-name leaf the chat LLM uses: `wf_<slug>`, where `slug_for_name`
/// maps `/` and `.` to `_` (and keeps alphanumerics + `-`). Mirrors
/// `modules::workflow_mcp::tools::slug_for_name`. The dev import names the
/// workflow `local.dev/<slug>`.
fn wf_tool_name(workflow_name: &str) -> String {
    let body: String = workflow_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("wf_{body}")
}

/// A user with the workflow perms needed for dev import + MCP execute.
async fn mcp_user(server: &TestServer, name: &str) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
        ],
    )
    .await
}

/// POST a JSON-RPC envelope to `/api/workflows/mcp`. When `conv_id` is set it's
/// echoed back via the `x-conversation-id` header (the path the chat MCP client
/// uses to scope the run to its conversation).
async fn jsonrpc(
    server: &TestServer,
    token: &str,
    conv_id: Option<Uuid>,
    method: &str,
    params: Json,
) -> reqwest::Response {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/workflows/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    if let Some(c) = conv_id {
        req = req.header("x-conversation-id", c.to_string());
    }
    req.send().await.expect("post workflow mcp jsonrpc")
}

/// Open a small pool for direct-SQL assertions.
async fn db_pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

#[tokio::test]
async fn initialize_and_tools_list_expose_the_workflow_as_a_tool() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_mcp_list").await;

    let wf = import_dev_workflow(&server, &user.token, "mcp-list", MCP_WORKFLOW_YAML).await;
    let wf_name = wf["name"].as_str().expect("workflow name");
    let expected_leaf = wf_tool_name(wf_name);

    // initialize → serverInfo.name = "workflow".
    let init = jsonrpc(&server, &user.token, None, "initialize", json!({})).await;
    assert_eq!(init.status(), 200);
    let init_body: Json = init.json().await.unwrap();
    assert_eq!(
        init_body["result"]["serverInfo"]["name"], "workflow",
        "initialize names the workflow server: {init_body}"
    );

    // tools/list → one tool whose composed name ends in the `wf_<slug>` leaf,
    // carrying the input schema derived from `inputs[]` (topic required).
    let list = jsonrpc(&server, &user.token, None, "tools/list", json!({})).await;
    assert_eq!(list.status(), 200);
    let list_body: Json = list.json().await.unwrap();
    let tools = list_body["result"]["tools"]
        .as_array()
        .expect("tools array");
    let tool = tools
        .iter()
        .find(|t| {
            t["name"]
                .as_str()
                .map(|n| n.ends_with(&expected_leaf))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("workflow tool '{expected_leaf}' in tools/list: {list_body}"));
    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("required array");
    assert!(
        required.iter().any(|v| v == "topic"),
        "input schema marks 'topic' required: {tool}"
    );
}

#[tokio::test]
async fn tools_call_from_conversation_spawns_conversation_sourced_run_and_completes() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_mcp_call").await;

    // A stub model + conversation so the MCP path's model SNAPSHOT succeeds (the
    // `call_tool` path resolves the model from the conversation; no real token
    // is spent — the sole llm step is mock-short-circuited).
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("wf-mcp conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    let wf = import_dev_workflow(&server, &user.token, "mcp-call", MCP_WORKFLOW_YAML).await;
    let wf_name = wf["name"].as_str().expect("workflow name");
    let leaf = wf_tool_name(wf_name);

    // tools/call wf_<slug> WITH the x-conversation-id header — simulating the
    // chat LLM invoking the workflow as a tool from inside its conversation.
    let resp = jsonrpc(
        &server,
        &user.token,
        Some(conv_id),
        "tools/call",
        json!({ "name": leaf, "arguments": { "topic": "espresso" } }),
    )
    .await;
    assert_eq!(resp.status(), 200, "tools/call should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "tools/call had no error: {body}");
    let result = &body["result"];
    assert_eq!(
        result["isError"], json!(false),
        "the formatted CallToolResult is a success: {result}"
    );
    // The formatted result inlines the (mocked) summary output.
    let text = result["content"][0]["text"]
        .as_str()
        .expect("result text body");
    assert!(
        text.contains("MCP_MOCK_SUMMARY"),
        "the call result carries the mocked summary output: {text}"
    );

    // The run was stamped invocation_source='conversation' and bound to the
    // originating conversation (the defining difference from the REST /run path).
    let run_id = Uuid::parse_str(
        result["structuredContent"]["metadata"]["run_id"]
            .as_str()
            .expect("run_id in result metadata"),
    )
    .expect("run_id uuid");

    let pool = db_pool(&server).await;
    let row = sqlx::query_as::<_, (String, Option<Uuid>, Option<Uuid>)>(
        "SELECT invocation_source, conversation_id, model_id FROM workflow_runs WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("run row");
    assert_eq!(
        row.0, "conversation",
        "the workflow_mcp tool-call path stamps invocation_source='conversation'"
    );
    assert_eq!(
        row.1,
        Some(conv_id),
        "the run is scoped to the originating conversation"
    );
    assert_eq!(row.2, Some(model_id), "the run snapshotted the conversation's model");
    pool.close().await;

    // The run reached a terminal completed status (the call blocks until terminal,
    // so this is already true, but assert via the REST read-back for clarity).
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the conversation-sourced run completed: {final_run}"
    );
}

#[tokio::test]
async fn tools_call_without_execute_permission_is_forbidden() {
    // The JSON-RPC handler is gated on `workflows::execute`. Migration 107 grants
    // `workflows::{read,execute}` to the default Users group, so a normal user
    // ALWAYS has execute — we must strip the user from ALL groups
    // (`create_user_with_no_permissions`) to actually exercise the 403 gate
    // (mirrors the web_search `test_tools_call_requires_use_permission` pattern).
    let server = TestServer::start().await;
    let stripped = create_user_with_no_permissions(&server, "wf_mcp_noperm").await;

    let resp = jsonrpc(
        &server,
        &stripped.token,
        None,
        "tools/call",
        json!({ "name": "wf_anything", "arguments": {} }),
    )
    .await;
    assert_eq!(
        resp.status(),
        403,
        "a caller lacking workflows::execute must be 403 on the workflow MCP server"
    );
}
