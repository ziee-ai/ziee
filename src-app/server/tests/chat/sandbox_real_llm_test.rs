//! Tier 5 — chat → real LLM → code_sandbox round-trip.
//!
//! Requires ALL of:
//!   - bwrap + mounted rootfs (see Tier 4 prerequisites)
//!   - ANTHROPIC_API_KEY env var
//!
//! Each test boots a TestServer with `code_sandbox.enabled: true`,
//! creates a user with an Anthropic-backed LLM model, sends a chat
//! message that nudges the LLM toward a specific sandbox tool, then
//! parses the SSE stream to verify the tool was invoked.
//!
//! Cost: each test ~10-30 cents in API tokens. Runs in the nightly
//! workflow only when explicitly enabled via workflow_dispatch input.

#![allow(unused_imports)]

use crate::chat::helpers;
use crate::code_sandbox::harness::github_fetch_server_options;
use crate::common::TestServer;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// Set the per-conversation MCP settings — auto-approval is applied
/// at the CONVERSATION level (not just the user_mcp_defaults level).
/// Returns the parsed response.
async fn set_conversation_mcp_settings(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    sandbox_id: Uuid,
    auto_approved_tools: &[&str],
) {
    let url = server.api_url(&format!("/conversations/{}/mcp-settings", conv_id));
    let resp = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [
                {
                    "server_id": sandbox_id,
                    "tools": auto_approved_tools,
                }
            ]
        }))
        .send()
        .await
        .expect("set mcp settings");
    let s = resp.status();
    assert!(
        s.is_success(),
        "mcp-settings PUT failed: {s} body: {:?}",
        resp.text().await
    );
}

/// Send a chat message with the code_sandbox MCP server explicitly
/// enabled for the conversation, then collect the reply frames off the
/// per-user chat stream. The chat module hides ALL MCP tools from the LLM
/// unless the request includes `enable_mcp: true` + a
/// `mcp_config.mcp_servers` list naming the desired server IDs.
///
/// Fire-and-forget: the POST to `/messages` returns `{user_message_id,
/// assistant_message_id}` (200, asserted inside the helper); the reply itself
/// streams over `GET /api/chat/stream`. `stop_at` lists event types at which
/// to stop short of a terminal — pass `["mcpApprovalRequired"]` for a flow
/// that pauses awaiting approval, or `&[]` to collect until `complete`/`error`.
/// Returns the collected frames as `SSEEvent`s (the `{event, data}` shape the
/// old per-request SSE response produced).
async fn send_with_sandbox_enabled(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    sandbox_id: Uuid,
    content: &str,
    stop_at: &[&str],
) -> Vec<helpers::SSEEvent> {
    let payload = json!({
        "content": content,
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": {
            "mcp_servers": [
                { "server_id": sandbox_id, "tools": [] }   // [] = all tools
            ]
        }
    });
    helpers::send_body_and_collect_events(server, token, conv_id, payload, stop_at).await
}

/// Skip if API key + bwrap aren't available. The rootfs is fetched from
/// the GitHub release by the server on first execute_command (shared
/// e2e cache), same as Tier-6 `enabled_test_server`.
async fn enabled_test_server_with_anthropic() -> Option<TestServer> {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return None;
    }
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    let opts = github_fetch_server_options(vec![("ANTHROPIC_API_KEY".into(), api_key)])?;
    Some(TestServer::start_with_options(opts).await)
}

/// Setup: register a user, create a conversation with a real
/// Anthropic-backed model, assign the built-in sandbox MCP server to
/// the user's test group (so the LLM can see the sandbox tools), and
/// seed the user_mcp_defaults so read_file/list_files/get_resource_link
/// are auto-approved (matching migration 36's behavior, which only
/// applies to pre-existing rows). Returns (user_token, user_id,
/// conv_id, branch_id, model_id).
async fn setup_chat_with_anthropic(
    server: &TestServer,
) -> (String, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let test_user =
        crate::common::test_helpers::create_user_with_permissions(server, "tier5_llm_user", &[
            "code_sandbox::execute",
            "llm_models::read",
            "llm_providers::read",
        ])
        .await;
    let user_id = Uuid::parse_str(&test_user.user_id).unwrap();

    // Wire the user's test group → sandbox MCP server so the LLM
    // sees the tools. Migration-time seeding only assigns the
    // sandbox to the DEFAULT Users group; the test group made by
    // create_user_with_permissions isn't that group.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let sandbox_id = ziee::code_sandbox::code_sandbox_server_id();
    // Find the test group this user was added to. `test_helpers::
    // create_user_with_permissions` creates ONE group per user named
    // `test_group_{8-hex-chars}` with the requested permissions set.
    // We must filter for that group SPECIFICALLY — the user is ALSO
    // a member of the default Users group (auto-assigned at
    // registration), and an earlier version of this query returned
    // the Users group instead, which made the subsequent INSERT a
    // no-op (Users already has the sandbox assigned at boot). The
    // Tier-5 tests then "passed" without proving the assignment did
    // anything.
    let group_id: Uuid = sqlx::query_scalar(
        "SELECT g.id FROM groups g \
         JOIN user_groups ug ON ug.group_id = g.id \
         WHERE ug.user_id = $1 \
           AND g.is_default = false \
           AND g.is_system = false \
           AND g.name LIKE 'test_group_%' \
         ORDER BY g.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("user must be in a custom test group");
    sqlx::query(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) \
         VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(sandbox_id)
    .execute(&pool)
    .await
    .expect("assign sandbox to test group");

    // Seed user_mcp_defaults with the read-only sandbox tools
    // auto-approved (mirrors migration 36, which only updates
    // EXISTING rows; this freshly-created user has no row yet).
    sqlx::query(
        r#"INSERT INTO user_mcp_defaults (user_id, auto_approved_tools, created_at, updated_at)
           VALUES ($1, jsonb_build_object($2::text, '["read_file","list_files","get_resource_link"]'::jsonb), NOW(), NOW())
           ON CONFLICT (user_id) DO UPDATE SET
               auto_approved_tools = jsonb_set(
                   COALESCE(user_mcp_defaults.auto_approved_tools, '{}'::jsonb),
                   ARRAY[$2::text],
                   '["read_file","list_files","get_resource_link"]'::jsonb,
                   true
               ),
               updated_at = NOW()"#,
    )
    .bind(user_id)
    .bind(sandbox_id.to_string())
    .execute(&pool)
    .await
    .expect("seed auto-approve defaults");
    pool.close().await;

    let model = helpers::get_or_create_test_model(server, &test_user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = helpers::create_conversation(server, &test_user.token, Some(model_id), Some("Tier-5 e2e"))
        .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    (test_user.token, user_id, conv_id, branch_id, model_id, sandbox_id)
}

/// `list_files` is auto-approved by migration 36. When the LLM
/// invokes it via MCP, no `mcpApprovalRequired` SSE event should
/// fire — the tool runs immediately and the result flows back.
#[tokio::test]
async fn list_files_via_llm_is_auto_approved() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id, sandbox_id) =
        setup_chat_with_anthropic(&server).await;
    // Auto-approve the read-only tools at the conversation level —
    // matches migration 36's intent for the per-conversation flow.
    set_conversation_mcp_settings(
        &server,
        &token,
        conv_id,
        sandbox_id,
        &["read_file", "list_files", "get_resource_link"],
    )
    .await;

    let events = send_with_sandbox_enabled(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        sandbox_id,
        "Call the `list_files` tool now to show me what's in the workspace. \
         Do not reply with text — call the tool. The workspace is empty \
         and that's fine; I just need to see the tool execute.",
        &[],
    )
    .await;
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();

    // KEY assertion 1: the tool ACTUALLY ran (not just "no approval
    // event"). mcpToolStart + mcpToolComplete fire whenever a tool
    // is invoked through the MCP path.
    let tool_starts: Vec<&helpers::SSEEvent> = events
        .iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert!(
        !tool_starts.is_empty(),
        "LLM did not call any tool. events: {event_names:?}"
    );
    let names: Vec<&str> = tool_starts
        .iter()
        .filter_map(|e| e.data["tool_name"].as_str())
        .collect();
    assert!(
        names.contains(&"list_files"),
        "LLM called other tools but not list_files: {names:?}"
    );

    // KEY assertion 2: auto-approved → no mcpApprovalRequired event.
    assert!(
        !event_names.contains(&"mcpApprovalRequired"),
        "list_files is auto-approved by migration 36; \
         mcpApprovalRequired MUST NOT appear. events: {event_names:?}"
    );
}

/// `read_file` is also auto-approved (migration 36). Same shape as
/// the list_files test.
#[tokio::test]
async fn read_file_via_llm_is_auto_approved() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id, sandbox_id) =
        setup_chat_with_anthropic(&server).await;
    set_conversation_mcp_settings(
        &server,
        &token,
        conv_id,
        sandbox_id,
        &["read_file", "list_files", "get_resource_link"],
    )
    .await;

    let events = send_with_sandbox_enabled(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        sandbox_id,
        // Note: file likely doesn't exist, but the tool call itself
        // happens before the file check. We assert the tool fired,
        // not that it succeeded.
        "Call the `read_file` tool now with filename 'README.txt'. \
         Do not reply with text — call the tool. If the file doesn't exist \
         that's expected; I just need to confirm the tool ran.",
        &[],
    )
    .await;
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();

    let tool_starts: Vec<&helpers::SSEEvent> = events
        .iter()
        .filter(|e| e.event == "mcpToolStart")
        .collect();
    assert!(
        !tool_starts.is_empty(),
        "LLM did not call any tool. events: {event_names:?}"
    );
    let names: Vec<&str> = tool_starts
        .iter()
        .filter_map(|e| e.data["tool_name"].as_str())
        .collect();
    assert!(
        names.contains(&"read_file"),
        "LLM called other tools but not read_file: {names:?}"
    );
    assert!(
        !event_names.contains(&"mcpApprovalRequired"),
        "read_file is auto-approved by migration 36; \
         mcpApprovalRequired MUST NOT appear. events: {event_names:?}"
    );
}

/// `execute_command` is NOT in the read-only auto-approve set
/// (migration 36 explicitly omits it). When the LLM invokes it, an
/// `mcpApprovalRequired` SSE event MUST fire. The test asserts the
/// event fires; it does NOT respond to it (the approval workflow is
/// covered by mcp_approval_workflow_test.rs).
#[tokio::test]
async fn execute_command_emits_approval_required_sse_event() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id, sandbox_id) =
        setup_chat_with_anthropic(&server).await;

    let events = send_with_sandbox_enabled(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        sandbox_id,
        // Forceful directive to maximize the chance Claude calls the
        // tool. Without "call the tool now" Claude tends to respond
        // conversationally for execute_command (which it knows is
        // destructive).
        "I have given you the `execute_command` tool. Call it RIGHT NOW \
         with command=\"echo hello-from-llm\". This is a test that needs \
         the tool to fire — do not respond with text, just invoke the \
         tool. The system will prompt me for approval; that's expected.",
        // execute_command is NOT auto-approved → the turn pauses at the
        // approval gate (no terminal frame) until a separate respond call,
        // which this test does not make. Stop collecting at the pause.
        &["mcpApprovalRequired"],
    )
    .await;
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
    // The KEY assertion: execute_command is NOT auto-approved, so an
    // mcpApprovalRequired event MUST fire. The test does NOT respond
    // to the approval (mcpToolStart will NOT fire). Full approval-
    // response flow is covered by mcp_approval_workflow_test.rs.
    assert!(
        event_names.contains(&"mcpApprovalRequired"),
        "execute_command MUST require approval but no mcpApprovalRequired \
         event seen — Claude may have refused to call the destructive tool. \
         events={event_names:?}"
    );
    // Sanity: mcpApprovalRequired event carries the tool_name field.
    let approval = events
        .iter()
        .find(|e| e.event == "mcpApprovalRequired")
        .expect("found above");
    assert_eq!(
        approval.data["tool_name"].as_str(),
        Some("execute_command"),
        "approval was for a different tool: {:?}",
        approval.data
    );
}

// =====================================================================
// LLM + a THIRD-PARTY MCP server running INSIDE the sandbox + sandbox.
//
// The tests above drive the BUILT-IN code_sandbox MCP server. This one
// closes the remaining gap: a `run_in_sandbox`-flagged stdio MCP server
// (a tiny python echo server) is spawned bwrap-isolated, the real LLM
// discovers its `echo` tool (which only works if the sandboxed child
// actually spawned + answered tools/list), invokes it, and the echoed
// result round-trips back through MCP → chat. Full chain:
//   LLM → MCP → run_in_sandbox spawn (bwrap) → python child → result.
// =====================================================================

/// Minimal stdio MCP server (echo only). `python3` ships in the
/// 'minimal' sandbox rootfs on every platform.
const SANDBOXED_ECHO_PY: &str = r#"
import json, sys
def respond(rid, result):
    sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":result})+"\n")
    sys.stdout.flush()
for raw in sys.stdin:
    raw = raw.strip()
    if not raw: continue
    req = json.loads(raw); m = req.get("method"); rid = req.get("id")
    if m == "initialize":
        respond(rid, {"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"tier5-echo","version":"0"}})
    elif m == "notifications/initialized":
        pass
    elif m == "tools/list":
        respond(rid, {"tools":[{"name":"echo","description":"Echo the msg argument straight back.","inputSchema":{"type":"object","properties":{"msg":{"type":"string"}},"required":["msg"]}}]})
    elif m == "tools/call":
        p = req.get("params", {})
        if p.get("name") == "echo":
            respond(rid, {"content":[{"type":"text","text": p.get("arguments",{}).get("msg","")}], "isError": False})
        else:
            respond(rid, {"content":[{"type":"text","text":"unknown tool"}], "isError": True})
    else:
        respond(rid, {})
"#;

/// Create a system stdio MCP server backed by `SANDBOXED_ECHO_PY`, with
/// `run_in_sandbox=true`. Returns the new server id.
async fn create_sandboxed_echo_mcp(server: &TestServer, admin_token: &str) -> Uuid {
    let url = server.api_url("/mcp/system-servers");
    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "name": format!("tier5-echo-{}", Uuid::new_v4()),
            "display_name": "Tier 5 Sandboxed Echo",
            "enabled": true,
            "transport_type": "stdio",
            "command": "python3",
            "args": ["-c", SANDBOXED_ECHO_PY],
            "environment_variables": {},
            "timeout_seconds": 60,
            "run_in_sandbox": true,
        }))
        .send()
        .await
        .expect("create system server");
    let status = resp.status();
    let body: Value = resp.json().await.expect("json");
    assert_eq!(status, 201, "create sandboxed echo server failed: {body}");
    Uuid::parse_str(body["id"].as_str().expect("server id")).unwrap()
}

/// Assign an MCP server to the custom test group the user belongs to, so
/// the chat module surfaces its tools to the LLM. Mirrors the group
/// lookup in `setup_chat_with_anthropic` (must target the
/// `test_group_%`, NOT the default Users group).
async fn assign_mcp_server_to_user_group(
    server: &TestServer,
    user_id: Uuid,
    mcp_server_id: Uuid,
) {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let group_id: Uuid = sqlx::query_scalar(
        "SELECT g.id FROM groups g \
         JOIN user_groups ug ON ug.group_id = g.id \
         WHERE ug.user_id = $1 \
           AND g.is_default = false AND g.is_system = false \
           AND g.name LIKE 'test_group_%' \
         ORDER BY g.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("user must be in a custom test group");
    sqlx::query(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) \
         VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(mcp_server_id)
    .execute(&pool)
    .await
    .expect("assign mcp server to test group");
    pool.close().await;
}

#[tokio::test]
async fn llm_drives_a_tool_on_a_sandboxed_mcp_server() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };

    // One user that can create the system server, see its tools, and chat.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tier5_sbxmcp",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers::read",
            "code_sandbox::execute",
            "llm_models::read",
            "llm_providers::read",
        ],
    )
    .await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    // Spawn-in-sandbox echo MCP server + make it visible to the LLM.
    let echo_id = create_sandboxed_echo_mcp(&server, &user.token).await;
    assign_mcp_server_to_user_group(&server, user_id, echo_id).await;

    let model = helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("Tier-5 sandboxed-mcp"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    // Auto-approve `echo` for this conversation so no approval gate fires.
    set_conversation_mcp_settings(&server, &user.token, conv_id, echo_id, &["echo"]).await;

    let sentinel = "SANDBOXED-ECHO-7F3A";
    let events = send_with_sandbox_enabled(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        echo_id,
        &format!(
            "Call the `echo` tool RIGHT NOW with msg=\"{sentinel}\". \
             Do not reply with text — just invoke the tool."
        ),
        &[],
    )
    .await;
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();

    // 1. The LLM discovered + invoked the sandboxed server's `echo` tool.
    //    tools/list only succeeds if the run_in_sandbox child actually
    //    spawned in bwrap and answered — so this proves the spawn path.
    let started: Vec<&str> = events
        .iter()
        .filter(|e| e.event == "mcpToolStart")
        .filter_map(|e| e.data["tool_name"].as_str())
        .collect();
    assert!(
        started.contains(&"echo"),
        "LLM did not call the sandboxed echo tool. starts={started:?} events={event_names:?}"
    );

    // 2. The echoed sentinel round-tripped from the bwrap-isolated child
    //    back through MCP → chat (proves real execution, not just discovery).
    let complete = events
        .iter()
        .find(|e| e.event == "mcpToolComplete" && e.data["tool_name"] == "echo")
        .unwrap_or_else(|| panic!("no mcpToolComplete for echo. events={event_names:?}"));
    assert_eq!(
        complete.data["is_error"], false,
        "sandboxed echo reported an error: {:?}",
        complete.data
    );
    let result = complete.data["result"].as_str().unwrap_or("");
    assert!(
        result.contains(sentinel),
        "echo result did not round-trip the sentinel through the sandbox. result={result:?}"
    );
}
