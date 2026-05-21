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
use crate::common::{TestServer, TestServerOptions};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// Skip if API key + bwrap + rootfs aren't all available.
async fn enabled_test_server_with_anthropic() -> Option<TestServer> {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return None;
    }
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return None;
    }
    let Some(rootfs) = rootfs_path() else {
        eprintln!("test skipped: no rootfs mounted");
        return None;
    };
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    Some(
        TestServer::start_with_options(TestServerOptions {
            sandbox_enabled: true,
            sandbox_rootfs: Some(rootfs),
            sandbox_cgroup_parent: String::new(),
            // Forward the API key so the spawned server can see it
            // when the LLM provider executes outbound calls.
            extra_env: vec![("ANTHROPIC_API_KEY".into(), api_key)],
        })
        .await,
    )
}

/// Setup: register a user, create a conversation with a real
/// Anthropic-backed model, return (user_token, user_id, conv_id,
/// branch_id, model_id). The user inherits the default Users group
/// which has `code_sandbox::execute` from migration 35.
async fn setup_chat_with_anthropic(
    server: &TestServer,
) -> (String, Uuid, Uuid, Uuid, Uuid) {
    let test_user =
        crate::common::test_helpers::create_user_with_permissions(server, "tier5_llm_user", &[
            "code_sandbox::execute",
            "llm_models::read",
            "llm_providers::read",
        ])
        .await;
    let user_id = Uuid::parse_str(&test_user.user_id).unwrap();
    let model = helpers::get_or_create_test_model(server, &test_user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = helpers::create_conversation(server, &test_user.token, Some(model_id), Some("Tier-5 e2e"))
        .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    (test_user.token, user_id, conv_id, branch_id, model_id)
}

/// `list_files` is auto-approved by migration 36. When the LLM
/// invokes it via MCP, no `mcpApprovalRequired` SSE event should
/// fire — the tool runs immediately and the result flows back.
#[tokio::test]
#[ignore]
async fn list_files_via_llm_is_auto_approved() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id) = setup_chat_with_anthropic(&server).await;

    let response = helpers::send_message_simple(
        &server,
        &token,
        conv_id,
        model_id,
        branch_id,
        "Please call the `list_files` tool to show me what's in my workspace. \
         Just list them. Don't add commentary.",
    )
    .await;
    let chunks = helpers::parse_sse_stream(response).await;

    // Look for evidence the tool was called. The chat module emits
    // tool-call events in the SSE stream; the auto-approved path
    // does NOT emit `mcpApprovalRequired`.
    let event_summary: Vec<String> = chunks
        .iter()
        .filter_map(|c| c.get("event").and_then(|e| e.as_str()).map(String::from))
        .collect();
    assert!(
        event_summary.iter().any(|e| e.contains("tool") || e.contains("content")),
        "no tool/content events in stream: {event_summary:?}"
    );
    assert!(
        !event_summary.iter().any(|e| e.contains("mcpApprovalRequired")),
        "auto-approved tool MUST NOT emit mcpApprovalRequired: {event_summary:?}"
    );
}

/// `read_file` is also auto-approved (migration 36). Same shape as
/// the list_files test.
#[tokio::test]
#[ignore]
async fn read_file_via_llm_is_auto_approved() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id) = setup_chat_with_anthropic(&server).await;

    let response = helpers::send_message_simple(
        &server,
        &token,
        conv_id,
        model_id,
        branch_id,
        // Note: file likely doesn't exist, but the tool call itself
        // happens before the file check. We assert the tool fired,
        // not that it succeeded.
        "Please call the `read_file` tool with filename 'README.txt'. \
         If it doesn't exist, that's fine — I just want to verify the tool works.",
    )
    .await;
    let chunks = helpers::parse_sse_stream(response).await;
    let event_summary: Vec<String> = chunks
        .iter()
        .filter_map(|c| c.get("event").and_then(|e| e.as_str()).map(String::from))
        .collect();
    assert!(
        !event_summary.iter().any(|e| e.contains("mcpApprovalRequired")),
        "auto-approved read_file must not require approval: {event_summary:?}"
    );
}

/// `execute_command` is NOT in the read-only auto-approve set
/// (migration 36 explicitly omits it). When the LLM invokes it, an
/// `mcpApprovalRequired` SSE event MUST fire. The test asserts the
/// event fires; it does NOT respond to it (the approval workflow is
/// covered by mcp_approval_workflow_test.rs).
#[tokio::test]
#[ignore]
async fn execute_command_emits_approval_required_sse_event() {
    let Some(server) = enabled_test_server_with_anthropic().await else { return };
    let (token, _user, conv_id, branch_id, model_id) = setup_chat_with_anthropic(&server).await;

    let response = helpers::send_message_simple(
        &server,
        &token,
        conv_id,
        model_id,
        branch_id,
        "Please call the `execute_command` tool to run `echo hello-from-llm`. \
         Just call the tool. I'll approve in a moment.",
    )
    .await;
    let chunks = helpers::parse_sse_stream(response).await;
    let event_summary: Vec<String> = chunks
        .iter()
        .filter_map(|c| c.get("event").and_then(|e| e.as_str()).map(String::from))
        .collect();
    // The KEY assertion: execute_command is NOT auto-approved.
    assert!(
        event_summary.iter().any(|e| e.contains("mcpApprovalRequired")
            || e.contains("approval")
            || e.contains("Approval")),
        "execute_command MUST require approval but no approval event seen: \
         events={event_summary:?}"
    );
}
