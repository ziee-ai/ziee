//! Real `kind: sandbox` (run-script) workflow step, end-to-end through the
//! full path: POST /run → runner → SandboxDispatcher → code_sandbox (bwrap)
//! → real shell command → captured stdout as the step output.
//!
//! Gated like the code_sandbox Tier-6 HTTP-E2E tests: needs a runnable
//! sandbox backend + a published rootfs for the host arch.
//! `harness::enabled_test_server()` returns `None` (clean skip) only when the
//! host genuinely can't run the sandbox (Linux without bubblewrap, or an
//! arch/tag with no published rootfs) — NOT a make-suite-green ignore, the
//! same genuine external dependency every sandbox tier gates on. Runs on
//! x86_64 Linux (bwrap) AND Apple-Silicon macOS (libkrun microVM), since
//! `v0.0.5-alpha` of `ziee-ai/sandbox-rootfs` publishes aarch64 squashfs too.

use serde_json::json;
use uuid::Uuid;

use super::{import_dev_workflow, poll_run, run_workflow, stub_conversation, workflow_user};

/// One sandbox step that echoes a templated input. `flavor: minimal` matches
/// the e2e rootfs (bash + coreutils — `echo` is enough).
const SANDBOX_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: name
    required: true
steps:
  - id: greet
    kind: sandbox
    run: echo "hello {{ inputs.name }} from the sandbox"
outputs:
  - name: greeting
    from: "{{ greet.output }}"
    expose: full
"#;

#[tokio::test]
async fn sandbox_run_script_workflow_completes() {
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        // Sandbox backend / rootfs unavailable for this host (no bubblewrap on
        // Linux, or an arch+tag with no published rootfs) — skip cleanly.
        return;
    };

    let user = workflow_user(&server, "wf_sandbox_user").await;
    // A conversation so the run can snapshot a model (unused — no llm steps —
    // but the run path snapshots conversation.model_id when conversation_id
    // is passed). Keep the stub guard alive.
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let wf = import_dev_workflow(&server, &user.token, "sandbox-echo", SANDBOX_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "name": "ziee" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "sandbox run-script workflow should complete; got: {final_run}"
    );

    // The sandbox step's captured stdout is its output file.
    let out = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/greet")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read greet output");
    assert_eq!(out.status(), 200, "sandbox output endpoint 200");
    let body = out.text().await.expect("output text");
    assert!(
        body.contains("hello ziee from the sandbox"),
        "sandbox echo ran and its stdout was captured; got: {body:?}"
    );
}

/// A sandbox-step output declared `expose: artifact` surfaces as a
/// `workflow_mcp` RESOURCE, and a chat model can recall it over the SAME
/// `/api/workflows/mcp` JSON-RPC path. This is the only test that combines the
/// **workflow MCP server** recall path WITH a real **sandbox** run (the
/// workflow_mcp suite uses a stub provider; the real-stack test never drives
/// the MCP resource path). Rootfs-gated like every sandbox tier.
const SANDBOX_MCP_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: name
    required: true
steps:
  - id: greet
    kind: sandbox
    run: echo "hello {{ inputs.name }} from the sandbox"
outputs:
  - name: greeting
    from: "{{ greet.output }}"
    expose: artifact
"#;

#[tokio::test]
async fn sandbox_run_output_is_recallable_via_workflow_mcp() {
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        return; // sandbox backend / rootfs unavailable — clean skip
    };

    let user = workflow_user(&server, "wf_sandbox_mcp_user").await;
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let wf = import_dev_workflow(
        &server,
        &user.token,
        "sandbox-mcp-echo",
        SANDBOX_MCP_WORKFLOW_YAML,
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "name": "ziee" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "sandbox+mcp workflow should complete; got: {final_run}"
    );

    // Drive the workflow_mcp JSON-RPC path (the same /api/workflows/mcp the chat
    // client uses) to RECALL the sandbox run's output as an MCP resource.
    let mcp = |method: &'static str, params: Json| {
        let url = server.api_url("/workflows/mcp");
        let token = user.token.clone();
        async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
                .send()
                .await
                .expect("workflow mcp jsonrpc")
        }
    };

    let expected_uri = format!("ziee://workflow-runs/{run_id}/outputs/greeting");

    // resources/list enumerates the completed run's artifact output.
    let list_body: Json = mcp("resources/list", json!({})).await.json().await.unwrap();
    assert!(list_body["error"].is_null(), "resources/list error: {list_body}");
    let listed = list_body["result"]["resources"]
        .as_array()
        .expect("resources array")
        .iter()
        .any(|r| r["uri"].as_str() == Some(expected_uri.as_str()));
    assert!(listed, "sandbox run output must be listed as an MCP resource: {list_body}");

    // resources/read returns the captured sandbox stdout.
    let read_resp = mcp("resources/read", json!({ "uri": expected_uri })).await;
    assert_eq!(read_resp.status(), 200, "resources/read should 200");
    let read_body: Json = read_resp.json().await.unwrap();
    let text = read_body["result"]["contents"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("hello ziee from the sandbox"),
        "recalled MCP resource must carry the sandbox stdout; got: {read_body}"
    );
}
