//! REAL-STACK combined workflow — EVERY step kind in ONE DAG, run end-to-end
//! against a REAL LLM (Groq-first):
//!
//!   plan (llm: emit a JSON list)
//!     → enrich (tool: a MockMcpServer registered as a user MCP server, returns
//!               structuredContent + a run-linked file via resource_link)
//!     → process (sandbox: kind:sandbox, flavor:minimal — consumes the prior
//!                LLM plan via stdin, processes it with a shell command, writes
//!                an artifact file)
//!     → signoff (elicit: human sign-off, answered mid-run)
//!     → synthesize (llm: synthesize the final output)
//!
//! This is the only workflow test that exercises all four dispatchers + the
//! elicit pause/resume in a single real run. It asserts the DURABLE artifacts
//! actually exist: the tool's run-linked `files` row (queried directly + via
//! `count_files_for_run`) AND the sandbox step's artifact file (fetched via the
//! run artifact endpoint → 200 + non-empty bytes). It also asserts
//! `final_output_json` carries every declared output and `step_outputs_json`
//! records all five steps.
//!
//! ── Two DIFFERENT gating philosophies (deliberate) ─────────────────────────
//! - The LLM key is HARD-REQUIRED. `get_or_create_groq_first_model` PANICS when
//!   no provider key is set — `tests/.env.test` ships working keys, so a
//!   real-LLM test must RUN, never silently pass
//!   (`feedback_no_ignore_unless_platform`). The combined run spends a few
//!   cents of real Groq tokens; that is intended.
//! - The SANDBOX is a legitimate PLATFORM dependency. `enabled_test_server()`
//!   returns `None` (clean skip) ONLY when the host genuinely can't run the
//!   sandbox (Linux without bubblewrap, or an arch/tag with no published
//!   rootfs) — same gate every code_sandbox Tier-6 test uses. That is the one
//!   acceptable skip here; the LLM key is not.

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    count_files_for_run, db_pool, import_dev_workflow, poll_run, register_mock_as_user_server,
    run_workflow,
};
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};

/// The combined workflow. `sandbox.flavor: minimal` matches the e2e rootfs
/// (bash + coreutils — `tr` / `wc` / `cat` are enough, no `jq`). The `process`
/// step consumes the `plan` LLM output via `stdin` (template-rendered to a
/// file, piped in), uppercases it, writes BOTH stdout (the step output) AND a
/// real artifact file under `artifacts/process/` (collected into
/// `step_artifacts_json` → fetchable via the run artifact endpoint).
///
/// Two `expose: artifact` outputs are declared:
///   - `plan_size`   → the sandbox step's stdout (the processed byte count);
///   - `enriched`    → the tool step's structuredContent (carries the run file).
/// (`expose: artifact` controls MCP presentation; the durable bytes are
/// asserted directly below.)
fn combined_workflow_yaml(server_name: &str) -> String {
    format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: topic
    required: true
steps:
  - id: plan
    kind: llm
    prompt: |
      List exactly 2 short keywords about "{{{{ inputs.topic }}}}" as a JSON
      array of strings. Return ONLY the JSON array, no prose.
    output_format: json
  - id: enrich
    kind: tool
    server: {server_name}
    tool: enrich
    arguments:
      keywords: "{{{{ plan.output }}}}"
    depends_on: [plan]
  - id: process
    kind: sandbox
    stdin: "{{{{ plan.output | json }}}}"
    run: >-
      cat | tr 'a-z' 'A-Z' | tee artifacts/process/upper.txt | wc -c
    depends_on: [enrich]
  - id: signoff
    kind: elicit
    message: "Processed {{{{ inputs.topic }}}}. Approve synthesis?"
    schema:
      type: object
      properties:
        approve:
          type: boolean
      required: [approve]
    timeout_ms: 120000
    depends_on: [process]
  - id: synthesize
    kind: llm
    prompt: |
      Write ONE short sentence about "{{{{ inputs.topic }}}}". The processed
      plan was {{{{ process.output }}}} bytes and approval was
      {{{{ signoff.output.approve }}}}.
    depends_on: [signoff]
outputs:
  - name: summary
    from: "{{{{ synthesize.output }}}}"
    expose: full
  - name: plan_size
    from: "{{{{ process.output }}}}"
    expose: artifact
  - name: enriched
    from: "{{{{ enrich.output }}}}"
    expose: artifact
"#
    )
}

/// A user holding the workflow perms PLUS the MCP + file perms the combined run
/// needs (register the mock as a user MCP server + persist the tool's run file).
async fn combined_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "mcp_servers::create",
            "mcp_servers::read",
            "files::upload",
            "files::read",
        ],
    )
    .await
}

/// Poll the run until it pauses on the elicit step, returning the
/// elicitation_id. Panics if the run terminates first or never pauses.
async fn wait_for_elicitation(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let run: Json = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("get run")
            .json()
            .await
            .expect("parse run");
        if let Some(p) = run["pending_elicitation_json"].as_object() {
            if let Some(id) = p.get("elicitation_id").and_then(|v| v.as_str()) {
                return Uuid::parse_str(id).expect("elicitation_id uuid");
            }
        }
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "completed" | "failed" | "cancelled") {
            panic!("run {run_id} reached '{status}' before pausing on elicit: {run}");
        }
        if std::time::Instant::now() >= deadline {
            panic!("run {run_id} never paused on elicit within 120s: {run}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn real_stack_combined_all_kinds_completes_with_durable_artifacts() {
    // Sandbox is a PLATFORM dependency — clean skip ONLY when the host can't run
    // it (no bubblewrap on Linux, or no published rootfs for this arch+tag).
    // This is the single acceptable skip; the LLM key is NOT (it panics).
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        eprintln!(
            "real_stack_combined: skipping — sandbox backend/rootfs unavailable on this host"
        );
        return;
    };

    let user = combined_user(&server, "wf_real_stack_user").await;

    // Real provider + model (Groq-first). PANICS if no key — no soft-skip.
    let model = crate::chat::helpers::get_or_create_groq_first_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().expect("model id")).expect("uuid");
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("real-stack workflow"),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("conv id").to_string();

    // The `tool` step's MCP server: returns structuredContent AND a run-linked
    // file via an `is_saved:false` resource_link (the dispatcher fetches the
    // bytes off the mock's byte route + persists a `files` row created_by=workflow).
    let mock = MockMcpServer::start().await;
    mock.on_download("enriched.csv", "text/csv", b"keyword,score\nalpha,1\nbeta,2\n");
    let dl_url = mock.download_url("enriched.csv");
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "enriched the plan" },
                {
                    "type": "resource_link",
                    "uri": dl_url,
                    "name": "enriched.csv",
                    "mimeType": "text/csv",
                    "is_saved": false,
                }
            ],
            "structuredContent": { "enriched": true, "rows": 2 },
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_real_stack_mock", &mock.base_url())
            .await;

    // Dev-import the combined workflow (no mocks — the llm + tool + sandbox steps
    // all run for real).
    let yaml = combined_workflow_yaml(&sname);
    let wf = import_dev_workflow(&server, &user.token, "real-stack-combined", &yaml).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // Kick the run (real provider; NO step mocks).
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "espresso coffee" },
            "conversation_id": conv_id,
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // plan (real llm) → enrich (real tool) → process (real sandbox) → pauses on
    // signoff (elicit). Answer it so synthesize (real llm) runs.
    let elicitation_id = wait_for_elicitation(&server, &user.token, run_id).await;
    let ack = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "response": { "approve": true } }))
        .send()
        .await
        .expect("submit elicit");
    assert_eq!(ack.status(), 200, "elicit submit should 200");

    // The run completes.
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "real-stack combined run should complete; got: {final_run}"
    );

    // ── per-step output metadata for ALL FIVE steps ──────────────────────────
    let outputs = &final_run["step_outputs_json"];
    for step in ["plan", "enrich", "process", "signoff", "synthesize"] {
        assert!(
            outputs.get(step).is_some(),
            "step '{step}' recorded output metadata: {outputs}"
        );
    }

    // ── final_output_json carries every declared output ──────────────────────
    let final_output = &final_run["final_output_json"];
    for name in ["summary", "plan_size", "enriched"] {
        assert!(
            final_output.get(name).is_some(),
            "final_output carries declared output '{name}': {final_run}"
        );
    }

    // ── DURABLE ARTIFACT 2: the sandbox step's artifact file ──────────────────
    // (Asserted BEFORE the tool-file check below so the sandbox-artifact
    // round-trip — the macOS virtio-fs-streaming fix — is proven independently.)
    // The sandbox wrote `artifacts/process/upper.txt`. On the Linux bwrap
    // backend the per-step artifacts mount is a host-backed bind, so the write
    // lands on the host fs → the runner's `collect_step_artifacts` finds it →
    // it's fetchable via the run artifact endpoint.
    //
    // macOS libkrun VM backend: libkrun's virtio-fs guest `open(O_CREAT)` fails
    // EPERM (broken credential switching; `docker/sbx-releases#51`,
    // `containers/podman#27679`), so a NEW file written to a virtio-fs RW bind
    // never reaches the host. The fix (this branch): the mac_vm backend binds a
    // guest-local **tmpfs** for each RW (artifact) mount and the guest agent
    // STREAMS the resulting files back over the existing vsock protocol
    // (`Frame::ArtifactFile`, sent before `Frame::Exit`); the host then writes
    // them into the real host artifact dir (its OWN fs — which works). So the
    // round-trip below now holds on EVERY backend.
    {
        let step_arts = final_run["step_artifacts_json"]["process"]
            .as_array()
            .unwrap_or_else(|| panic!("process step has collected artifacts: {final_run}"));
        assert!(
            !step_arts.is_empty(),
            "the sandbox step produced at least one artifact: {final_run}"
        );
        let art_name = step_arts[0]["filename"]
            .as_str()
            .expect("artifact filename");
        assert_eq!(art_name, "upper.txt", "the sandbox wrote upper.txt");

        let art_resp = reqwest::Client::new()
            .get(server.api_url(&format!(
                "/workflow-runs/{run_id}/artifact/process/{art_name}"
            )))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("read sandbox artifact");
        assert_eq!(art_resp.status(), 200, "sandbox artifact endpoint 200");
        let art_bytes = art_resp.bytes().await.expect("artifact bytes");
        assert!(
            !art_bytes.is_empty(),
            "the sandbox artifact file is non-empty"
        );
        // The sandbox uppercased the plan; the artifact must contain uppercase JSON.
        let art_text = String::from_utf8_lossy(&art_bytes);
        assert!(
            art_text.chars().any(|c| c.is_ascii_uppercase()),
            "the sandbox artifact carries the uppercased plan: {art_text:?}"
        );
    }
    // The sandbox step's STDOUT (its byte count) also round-tripped — a second,
    // independent durability check (the durable output, separate from the
    // artifact-file mount). Holds on every backend.
    {
        let out_resp = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}/output/process")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("read process output");
        assert_eq!(out_resp.status(), 200, "sandbox step output endpoint 200");
        let out_body = out_resp.text().await.expect("process output text");
        assert!(
            out_body.trim().chars().all(|c| c.is_ascii_digit()) && !out_body.trim().is_empty(),
            "the sandbox step's STDOUT (a byte count) round-tripped: {out_body:?}"
        );
    }

    // ── DURABLE ARTIFACT 1: the tool step's run-linked file actually exists ───
    // TWO workflow-authored files are run-linked now (both via the A3/A5
    // durable-artifact path → `files.workflow_run_id` FK, `created_by="workflow"`):
    //   - the tool step's `enriched.csv` (MCP `resource_link` → persist_links), and
    //   - the sandbox step's `upper.txt` (collected via the mac_vm tmpfs+vsock
    //     artifact fix proven in DURABLE ARTIFACT 2; on Linux it was always
    //     collected via the host bind).
    // Before the mac_vm fix the sandbox file silently vanished on macOS, so this
    // count was 1 there; it is 2 on every backend now. (A count of 0/1 here while
    // DURABLE ARTIFACT 2 passes would indicate a regression in the tool-file FK
    // path `resource_link::set_workflow_run_id`, NOT the sandbox streaming.)
    let pool = db_pool(&server).await;
    assert_eq!(
        count_files_for_run(&pool, run_id, "workflow").await,
        2,
        "both workflow-authored files are run-linked: the tool's enriched.csv + the sandbox's upper.txt"
    );
    // Fetch the TOOL file specifically (the resource_link-FK provenance check);
    // there are two run-linked files now, so filter by filename.
    let file_row = sqlx::query_as::<_, (Uuid, String, Option<Uuid>)>(
        "SELECT id, created_by, workflow_run_id FROM files \
         WHERE workflow_run_id = $1 AND filename = 'enriched.csv'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("tool-step run-linked file row (enriched.csv)");
    let file_id = file_row.0;
    assert_eq!(file_row.1, "workflow", "file authored by the workflow");
    assert_eq!(file_row.2, Some(run_id), "file links to the run");
    pool.close().await;

    // The file's bytes are downloadable + non-empty (the blob really persisted).
    let file_resp = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/download")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("download run file");
    assert_eq!(file_resp.status(), 200, "run-linked file downloads 200");
    let file_bytes = file_resp.bytes().await.expect("file bytes");
    assert!(!file_bytes.is_empty(), "the persisted run file is non-empty");

    // ── the real synthesized summary is non-trivial ──────────────────────────
    let summary_resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/synthesize")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read synthesize output");
    assert_eq!(summary_resp.status(), 200, "synthesize output endpoint 200");
    let summary = summary_resp.text().await.expect("summary text");
    assert!(
        summary.trim().len() > 10,
        "real LLM produced a non-trivial summary; got: {summary:?}"
    );
}
