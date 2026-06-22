//! Live sandbox-progress path — integration + real-stack E2E.
//!
//! Two tiers here (the consumer's coalesce/cap/drop/flush policy is covered by
//! in-source unit tests in `workflow::sandbox_progress`, since that module isn't
//! re-exported from the crate root):
//!
//! 1. `snapshot_carries_step_progress_and_manifest_descriptions` — the P2.6
//!    refresh contract, through the REAL `GET /workflow-runs/{id}/events` SSE
//!    endpoint + raw DB. No sandbox/VM: it asserts the Snapshot frame projects
//!    the persisted `step_progress_json` verbatim AND carries the pipeline
//!    `step_manifest` with each step's (raw) `description` template.
//!
//! 2. `real_sandbox_step_streams_live_progress` — the full guest→host→SSE path:
//!    a `kind: sandbox` step writes `progress.v1` lines to `$ZIEE_PROGRESS`; a
//!    concurrent SSE subscriber must observe live `stepProgress` `bar` frames.
//!    Gated on `code_sandbox::harness::enabled_test_server()` — the same genuine
//!    platform dependency every sandbox tier gates on (clean skip when no
//!    bwrap/rootfs), NOT a make-suite-green ignore. On Apple-Silicon macOS this
//!    needs a guest-root bundle built from the current agent source (the FIFO
//!    reader); CI/release rebuilds it from source on every build.

use std::time::Duration;

use serde_json::{Value as Json, json};
use uuid::Uuid;

use crate::common::TestServer;

use super::{
    db_pool, import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation,
    workflow_user,
};

/// Parse one SSE frame (`event:`/`data:` lines) into `(event_name, data_json)`.
/// Tolerant of an optional space after the colon (servers differ).
fn parse_sse_frame(frame: &str) -> Option<(String, String)> {
    let mut ev = None;
    let mut data = None;
    for line in frame.lines() {
        if let Some(r) = line.strip_prefix("event:") {
            ev = Some(r.trim().to_string());
        } else if let Some(r) = line.strip_prefix("data:") {
            data = Some(r.trim().to_string());
        }
    }
    Some((ev?, data?))
}

/// Open the per-run SSE stream and return the first `snapshot` frame's data.
async fn read_snapshot(server: &TestServer, token: &str, run_id: Uuid) -> Json {
    let mut resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/events")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("subscribe to run events");
    assert_eq!(resp.status(), 200, "events endpoint should 200 for an owned run");

    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let chunk = tokio::time::timeout_at(deadline, resp.chunk())
            .await
            .expect("timed out waiting for snapshot frame")
            .expect("chunk read error");
        let Some(bytes) = chunk else {
            break; // stream closed
        };
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(idx) = buf.find("\n\n") {
            let frame: String = buf.drain(..idx + 2).collect();
            if let Some((ev, data)) = parse_sse_frame(&frame) {
                if ev == "snapshot" {
                    return serde_json::from_str(&data).expect("parse snapshot data json");
                }
            }
        }
    }
    panic!("SSE stream closed before a snapshot frame arrived");
}

/// Subscribe to the per-run SSE stream and collect every `stepProgress` frame's
/// data until a terminal frame arrives (or `timeout`). Retries the initial
/// connect briefly to ride out the window between the `202` and the runner
/// registering the run handle.
async fn collect_step_progress(
    server: &TestServer,
    token: &str,
    run_id: Uuid,
    timeout: Duration,
) -> Vec<Json> {
    let url = server.api_url(&format!("/workflow-runs/{run_id}/events"));
    let client = reqwest::Client::new();

    let mut resp = None;
    for _ in 0..40 {
        match client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
        {
            Ok(r) if r.status() == 200 => {
                resp = Some(r);
                break;
            }
            // 404 WORKFLOW_RUN_NOT_ACTIVE: handle not registered yet — retry.
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let mut resp = match resp {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let chunk = match tokio::time::timeout_at(deadline, resp.chunk()).await {
            Ok(Ok(Some(c))) => c,
            _ => break, // timeout, stream end, or error
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find("\n\n") {
            let frame: String = buf.drain(..idx + 2).collect();
            if let Some((ev, data)) = parse_sse_frame(&frame) {
                match ev.as_str() {
                    "stepProgress" => {
                        if let Ok(d) = serde_json::from_str::<Json>(&data) {
                            out.push(d);
                        }
                    }
                    "runCompleted" | "runFailed" | "runCancelled" => return out,
                    _ => {}
                }
            }
        }
    }
    out
}

/// A single llm step carrying a templated `description` (raw template surfaces in
/// the snapshot manifest). Mocked so it completes with no provider.
const DESC_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: research
    kind: llm
    description: "Research {{ inputs.topic }} in depth"
    prompt: "Find sources on {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ research.output }}"
    expose: full
"#;

#[tokio::test]
async fn snapshot_carries_step_progress_and_manifest_descriptions() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_progress_snapshot").await;

    let wf = import_dev_workflow(&server, &user.token, "progress-desc", DESC_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    // A completed (terminal) mocked run → the events endpoint replays
    // Connected + Snapshot then closes (no live registry handle required).
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "tardigrades" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "research": "canned research output" }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "mocked run completes: {final_run}");

    // Simulate an in-flight progress snapshot persisted on the run row (P2.6):
    // the refresh path must surface it verbatim.
    let pool = db_pool(&server).await;
    let tracks = json!({
        "dl": {"id":"dl","label":"Downloading","done":false,"kind":{"type":"bar","fraction":0.5}},
        "st": {"id":"st","done":false,"kind":{"type":"status","message":"working"}}
    });
    sqlx::query("UPDATE workflow_runs SET step_progress_json = $1 WHERE id = $2")
        .bind(&tracks)
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("persist step_progress_json");

    let snapshot = read_snapshot(&server, &user.token, run_id).await;

    // (a) refresh rehydrates the in-flight tracks verbatim.
    assert_eq!(
        snapshot["step_progress_json"], tracks,
        "snapshot projects persisted progress verbatim: {snapshot}"
    );

    // (b) the pipeline manifest carries every step + its RAW description template
    //     (the FE renders/upgrades it as StepStarted frames arrive).
    let manifest = snapshot["step_manifest"]
        .as_array()
        .expect("step_manifest is an array");
    let research = manifest
        .iter()
        .find(|m| m["id"] == "research")
        .unwrap_or_else(|| panic!("'research' step in manifest: {snapshot}"));
    assert_eq!(
        research["description"], "Research {{ inputs.topic }} in depth",
        "manifest carries the raw description template: {snapshot}"
    );
    assert_eq!(research["kind"], "llm", "manifest carries the step kind");
}

/// A `kind: sandbox` step that writes 5 `progress.v1` bar updates to
/// `$ZIEE_PROGRESS` over ~2s, then prints a final line as its output.
const PROGRESS_SANDBOX_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: name
    required: true
steps:
  - id: work
    kind: sandbox
    description: "Crunch {{ inputs.name }}"
    run: |
      for i in 1 2 3 4 5; do
        printf '{"type":"bar","id":"work","label":"crunching","fraction":0.%s}\n' "$i" > "$ZIEE_PROGRESS"
        sleep 0.4
      done
      echo "done crunching {{ inputs.name }}"
outputs:
  - name: log
    from: "{{ work.output }}"
    expose: full
"#;

#[tokio::test]
async fn real_sandbox_step_streams_live_progress() {
    // Genuine platform dependency (bwrap on Linux / libkrun VM on macOS + a
    // published rootfs). Clean skip when unavailable — NOT a green-the-suite
    // ignore.
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        eprintln!(
            "real_sandbox_step_streams_live_progress: skipping — sandbox backend/rootfs unavailable on this host"
        );
        return;
    };

    let user = workflow_user(&server, "wf_live_progress").await;
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let wf = import_dev_workflow(&server, &user.token, "live-progress", PROGRESS_SANDBOX_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "name": "espresso" },
            "conversation_id": conv_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // Subscribe right after 202 and collect live stepProgress frames until the
    // run terminates. The first execute_command may fetch+mount the rootfs
    // (slow), so allow a generous overall budget.
    let collected = collect_step_progress(&server, &user.token, run_id, Duration::from_secs(240)).await;

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "sandbox progress run completes: {final_run}"
    );

    // At least one live `bar` track for step `work` must have flowed
    // guest→host→SSE — proving the FIFO → ProcessProgress → consumer → SSE path.
    let saw_work_bar = collected.iter().any(|d| {
        d["step_id"] == "work"
            && d["tracks"]
                .as_array()
                .map(|ts| ts.iter().any(|t| t["id"] == "work" && t["kind"]["type"] == "bar"))
                .unwrap_or(false)
    });
    assert!(
        saw_work_bar,
        "expected a live stepProgress 'bar' track for step 'work'; got {} frame(s): {:?}",
        collected.len(),
        collected
    );
}
