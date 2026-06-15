//! Real `kind: sandbox` (run-script) workflow step, end-to-end through the
//! full path: POST /run → runner → SandboxDispatcher → code_sandbox (bwrap)
//! → real shell command → captured stdout as the step output.
//!
//! Gated like the code_sandbox Tier-6 HTTP-E2E tests: needs bwrap + a
//! published rootfs. `harness::enabled_test_server()` returns `None` (clean
//! skip) when the host can't run the sandbox (no bwrap, or non-x86_64 where
//! no rootfs asset is published) — NOT a make-suite-green ignore, the same
//! genuine external dependency every sandbox tier gates on. Runs in CI on
//! x86_64 Linux with bubblewrap installed.

use serde_json::{Value as Json, json};
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
        // bwrap/rootfs unavailable (e.g. non-x86_64 or no bubblewrap) — skip.
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
