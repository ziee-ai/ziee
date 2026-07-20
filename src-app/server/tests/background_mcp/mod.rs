//! background_mcp — spawn a DETACHED sub-agent run FROM A CONVERSATION via the
//! built-in `POST /api/background/mcp` JSON-RPC server, and prove the executor
//! now drives a REAL `AgentCore` turn (tranche 10b) — not the tranche-10
//! `minimal-placeholder`.
//!
//! The single test walks the full lifecycle end-to-end with NO real LLM key:
//!   - a stub model + conversation supply the run's model (`create_stub_model`
//!     returns the canned assistant text `"Hello from stub"`);
//!   - `tools/call spawn_background {spec:{task}}` (+ `x-conversation-id`)
//!     launches the detached run and returns an opaque `run_id`;
//!   - `check_status` is polled until the run is `terminal` + `completed`;
//!   - `collect_result` returns a `final_output` whose `executor` is
//!     `"agent-core"` (NOT `"minimal-placeholder"`) and whose `final_text`
//!     carries the stub's real assistant answer.
//!
//! This is the background analog of `workflow_mcp`'s conversation-sourced run
//! test, and the background twin of `workflow/agent_step_test.rs` (which proves
//! the SAME shared `AgentCore` loop drives a workflow `kind: agent` step). Both
//! hosts now build their core through `agent_dispatch::build_detached_agent_core`.

mod run_notes;
mod runs;

use std::time::{Duration, Instant};

use serde_json::Value as Json;
use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// A user that can reach the background tools. `background::use` is granted to the
/// default Users group at runtime, but the test-only `create_user_with_permissions`
/// grants an explicit set, so it's listed here directly.
async fn background_user(server: &TestServer, name: &str) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(server, name, &["background::use"]).await
}

/// POST a JSON-RPC envelope to `/api/background/mcp`. When `conv_id` is set it's
/// echoed via `x-conversation-id` (the path the chat MCP client uses to scope the
/// run to its conversation — and, for `spawn_background`, to resolve the model).
async fn jsonrpc(
    server: &TestServer,
    token: &str,
    conv_id: Option<Uuid>,
    method: &str,
    params: Json,
) -> Json {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/background/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    if let Some(c) = conv_id {
        req = req.header("x-conversation-id", c.to_string());
    }
    let resp = req.send().await.expect("post background mcp jsonrpc");
    assert_eq!(resp.status(), 200, "background mcp jsonrpc should 200");
    resp.json().await.expect("parse jsonrpc response")
}

/// Extract the tool's `structuredContent` from a successful `tools/call`.
fn structured(body: &Json) -> &Json {
    assert!(body["error"].is_null(), "tools/call had no error: {body}");
    &body["result"]["structuredContent"]
}

#[tokio::test]
async fn spawn_background_runs_a_real_agent_turn_to_completion() {
    let server = TestServer::start().await;
    let user = background_user(&server, "bg_mcp_real_turn").await;

    // A stub model + conversation → the detached sub-agent resolves + runs on it
    // (no real token spent; the stub returns the canned "Hello from stub").
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("bg-mcp conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    // tools/call spawn_background WITH x-conversation-id — the chat LLM launching a
    // detached sub-agent from inside its conversation.
    let spawn = jsonrpc(
        &server,
        &user.token,
        Some(conv_id),
        "tools/call",
        json!({
            "name": "spawn_background",
            "arguments": { "spec": { "task": "Say a one-line hello." } }
        }),
    )
    .await;
    let sc = structured(&spawn);
    assert_eq!(sc["status"], "pending", "spawn returns a pending handle: {sc}");
    let run_id = sc["run_id"].as_str().expect("run_id in spawn result").to_string();

    // Poll check_status (owner-scoped read, approval-bypassed) until terminal.
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        let body = jsonrpc(
            &server,
            &user.token,
            Some(conv_id),
            "tools/call",
            json!({ "name": "check_status", "arguments": { "run_id": run_id } }),
        )
        .await;
        let sc = structured(&body);
        if sc["terminal"].as_bool().unwrap_or(false) {
            break sc.clone();
        }
        assert!(
            Instant::now() < deadline,
            "background run did not reach terminal in 30s: {sc}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert_eq!(
        status["status"], "completed",
        "the detached sub-agent run should complete (real AgentCore turn): {status}"
    );

    // collect_result → the REAL turn's output, NOT the tranche-10 placeholder.
    let collect = jsonrpc(
        &server,
        &user.token,
        Some(conv_id),
        "tools/call",
        json!({ "name": "collect_result", "arguments": { "run_id": run_id } }),
    )
    .await;
    let sc = structured(&collect);
    assert_eq!(sc["complete"], json!(true), "collect_result is complete: {sc}");
    let chunk = sc["final_output_chunk"]
        .as_str()
        .expect("final_output_chunk in collect_result");
    assert!(
        chunk.contains("agent-core"),
        "final_output is produced by the real agent-core executor: {chunk}"
    );
    assert!(
        !chunk.contains("minimal-placeholder"),
        "the tranche-10 placeholder must be GONE: {chunk}"
    );
    assert!(
        chunk.contains("Hello from stub"),
        "final_text carries the stub model's real assistant answer: {chunk}"
    );
}
