//! Group C — background SANDBOX code execution, end-to-end (ITEM-11/12/13).
//!
//! Spawn a DETACHED sandbox command FROM A CONVERSATION via the built-in
//! `POST /api/background/mcp` JSON-RPC server, and prove the `JobKind::SandboxExec`
//! executor drives a `workflow_runs` row to `completed` with the command's real
//! stdout + exit_code captured in `final_output_json` — collectible later via the
//! same `check_status` / `collect_result` reads that serve every background kind.
//!
//! Rootfs-gated + `#[ignore]`'d, mirroring the code_sandbox tier6 tests: it runs a
//! REAL bwrap command, so it reuses the sandbox-enabled server harness
//! (`code_sandbox::harness::enabled_test_server`) which self-skips cleanly when the
//! host can't run the sandbox (no bwrap / no published rootfs for this arch). The
//! rootfs-FREE executor wiring (final_output projection + notification summary +
//! kind routing) is proven WITHOUT a live sandbox by the unit tests in
//! `modules/background_mcp/tools.rs`.

use std::time::{Duration, Instant};

use serde_json::json;
use uuid::Uuid;

use super::{background_user, jsonrpc, structured};

#[tokio::test]
#[ignore = "needs a booted code_sandbox + published rootfs (bwrap); mirrors tier6 — run explicitly"]
async fn spawn_sandbox_exec_runs_a_command_to_completion() {
    // Sandbox-enabled server (fetches the pinned rootfs on first execute_command).
    // Skips cleanly when the host genuinely can't run the sandbox.
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        eprintln!("test skipped: code_sandbox not runnable on this host");
        return;
    };

    let user = background_user(&server, "bg_sandbox_exec").await;

    // A conversation the user owns — the per-conversation sandbox workspace the
    // background command runs in (no model needed for a plain command).
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        None,
        Some("bg-sandbox conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    // spawn_background{kind:sandbox_exec, spec:{command}} WITH x-conversation-id —
    // the chat LLM launching a detached sandbox command from its conversation.
    let spawn = jsonrpc(
        &server,
        &user.token,
        Some(conv_id),
        "tools/call",
        json!({
            "name": "spawn_background",
            "arguments": {
                "kind": "sandbox_exec",
                "spec": { "command": "echo ziee-bg-hello" }
            }
        }),
    )
    .await;
    let sc = structured(&spawn);
    assert_eq!(sc["status"], "pending", "spawn returns a pending handle: {sc}");
    assert_eq!(sc["kind"], "sandbox_exec", "spawn echoes the sandbox kind: {sc}");
    let run_id = sc["run_id"].as_str().expect("run_id in spawn result").to_string();

    // Poll check_status (owner-scoped read, approval-bypassed) until terminal. A
    // first-run rootfs fetch + the command can take a while → generous deadline.
    let deadline = Instant::now() + Duration::from_secs(300);
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
            "background sandbox run did not reach terminal in 300s: {sc}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    assert_eq!(
        status["status"], "completed",
        "the detached sandbox command run should complete: {status}"
    );

    // collect_result → final_output carries the command's real stdout + exit_code.
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
        chunk.contains("code-sandbox"),
        "final_output is produced by the code-sandbox executor: {chunk}"
    );
    assert!(
        chunk.contains("ziee-bg-hello"),
        "the command's stdout is captured in final_output: {chunk}"
    );
    assert!(
        chunk.contains("\"exit_code\":0"),
        "exit_code 0 captured in final_output: {chunk}"
    );
}
