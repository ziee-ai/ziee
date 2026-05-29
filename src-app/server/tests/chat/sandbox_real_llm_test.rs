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
use crate::code_sandbox::harness::{bwrap_available, rootfs_path};
// stage_test_rootfs_for_e2e is only defined (and only used, at the call site
// below) on macOS/Windows; importing it unconditionally fails to compile on
// Linux where the fn is cfg'd out.
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::code_sandbox::harness::stage_test_rootfs_for_e2e;
use crate::common::{TestServer, TestServerOptions};
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
/// enabled for the conversation. The chat module hides ALL MCP tools
/// from the LLM unless the request includes `enable_mcp: true` + a
/// `mcp_config.mcp_servers` list naming the desired server IDs.
async fn send_with_sandbox_enabled(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    sandbox_id: Uuid,
    content: &str,
) -> reqwest::Response {
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
    let url = server.api_url(&format!("/conversations/{}/messages/stream", conv_id));
    reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("send message")
}

/// Skip if API key + bwrap + rootfs aren't all available. On Linux uses
/// the host-mounted production rootfs; on Mac/Windows stages the test
/// squashfs via the same cross-platform path as Tier-6 `enabled_test_server`.
async fn enabled_test_server_with_anthropic() -> Option<TestServer> {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return None;
    }
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();

    #[cfg(target_os = "linux")]
    {
        if !bwrap_available() {
            eprintln!("test skipped: bwrap not installed");
            return None;
        }
        let Some(rootfs) = rootfs_path() else {
            eprintln!("test skipped: no rootfs mounted");
            return None;
        };
        return Some(
            TestServer::start_with_options(TestServerOptions {
                sandbox_enabled: true,
                rate_limit: None,
                sandbox_rootfs: Some(rootfs),
                sandbox_cgroup_parent: String::new(),
                extra_env: vec![("ANTHROPIC_API_KEY".into(), api_key)],
                sandbox_cache_tempdir: None,
            })
            .await,
        );
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let (cache, mut env) = stage_test_rootfs_for_e2e()
            .expect("stage test rootfs (run `just test-prereqs`)");
        let rootfs_path = cache.path().join("current");
        env.push(("ANTHROPIC_API_KEY".into(), api_key));
        return Some(
            TestServer::start_with_options(TestServerOptions {
                sandbox_enabled: true,
                rate_limit: None,
                sandbox_rootfs: Some(rootfs_path),
                sandbox_cgroup_parent: String::new(),
                extra_env: env,
                sandbox_cache_tempdir: Some(std::sync::Arc::new(cache)),
            })
            .await,
        );
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = api_key;
        eprintln!("test skipped: unsupported platform");
        None
    }
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

    let response = send_with_sandbox_enabled(
        &server,
        &token,
        conv_id,
        branch_id,
        model_id,
        sandbox_id,
        "Call the `list_files` tool now to show me what's in the workspace. \
         Do not reply with text — call the tool. The workspace is empty \
         and that's fine; I just need to see the tool execute.",
    )
    .await;
    let events = helpers::parse_sse_events(response).await;
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

    let response = send_with_sandbox_enabled(
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
    )
    .await;
    let events = helpers::parse_sse_events(response).await;
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

    let response = send_with_sandbox_enabled(
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
    )
    .await;
    let events = helpers::parse_sse_events(response).await;
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
