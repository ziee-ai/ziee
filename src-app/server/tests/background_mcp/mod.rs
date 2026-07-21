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
// Group C — background sandbox code execution (ITEM-11/12/13). Rootfs-gated
// (mirrors the code_sandbox tier6 pattern): the driver runs a REAL bwrap command,
// so it needs a booted sandbox + a published rootfs. Linux is the reference
// bwrap path. The rootfs-FREE executor wiring (row/notification/serialization) is
// proven by the `background_mcp::tools` unit tests.
#[cfg(target_os = "linux")]
mod sandbox;

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

/// Insert a REAL classic workflow run (`job_kind='workflow'`) owned by `user_id`.
/// A workflow-kind run requires a non-NULL `workflow_id` (the coherence guard), so
/// a real `workflows` bundle row is inserted first. Used to prove the background
/// MCP reads enforce the `job_kind <> 'workflow'` boundary (a caller's OWN
/// workflow run must 404 from `check_status` / `collect_result`).
async fn insert_workflow_run(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let owner = Uuid::parse_str(user_id).unwrap();
    let workflow_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workflows \
            (id, name, extracted_path, bundle_sha256, bundle_size_bytes, file_count, entry_point, scope, owner_user_id) \
         VALUES ($1, $2, '/tmp/bg-mcp-boundary', 'deadbeef', 0, 0, 'workflow.yaml', 'user', $3)",
    )
    .bind(workflow_id)
    .bind(format!("bg-mcp-wf-{}", workflow_id.simple()))
    .bind(owner)
    .execute(&pool)
    .await
    .expect("insert workflow bundle");
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workflow_runs (workflow_id, user_id, job_kind, status) \
         VALUES ($1, $2, 'workflow', 'running') RETURNING id",
    )
    .bind(workflow_id)
    .bind(owner)
    .fetch_one(&pool)
    .await
    .expect("insert workflow run")
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

/// FINDING-2 (background-boundary): the background MCP READS (`check_status` /
/// `collect_result`) enforce the `job_kind <> 'workflow'` boundary — a caller's
/// OWN classic workflow run is 404'd (surfaced as a JSON-RPC error), never read
/// through the background surface (owner-scoped, so no cross-user leak either; the
/// point is the workflow/background boundary). Mirrors the list/detail endpoints.
#[tokio::test]
async fn background_reads_reject_own_workflow_run() {
    let server = TestServer::start().await;
    let user = background_user(&server, "bg_mcp_wf_boundary").await;
    // The caller's OWN classic workflow run — NOT a background run.
    let run_id = insert_workflow_run(&server, &user.user_id).await;

    // check_status → JSON-RPC error (404), no leaked result.
    let status = jsonrpc(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "check_status", "arguments": { "run_id": run_id.to_string() } }),
    )
    .await;
    assert!(
        !status["error"].is_null(),
        "check_status of one's OWN workflow run must be a 404 error (background-only): {status}"
    );
    assert!(
        status["result"].is_null(),
        "no run state leaked for a workflow run via check_status: {status}"
    );

    // collect_result → JSON-RPC error (404), no leaked result.
    let collect = jsonrpc(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "collect_result", "arguments": { "run_id": run_id.to_string() } }),
    )
    .await;
    assert!(
        !collect["error"].is_null(),
        "collect_result of one's OWN workflow run must be a 404 error (background-only): {collect}"
    );
    assert!(
        collect["result"].is_null(),
        "no result leaked for a workflow run via collect_result: {collect}"
    );
}
