//! audit id all-3d4cdb88bfa6 — workflow run SSE event ORDERING + COMPLETENESS.
//! sandbox_progress.rs asserts the snapshot + stepProgress bar shapes but never
//! the stream-level guarantees: the stream opens with a `snapshot` frame
//! (ordering) and delivers the run's terminal `completed` status (completeness).
//! Driven by a mocked single-step run (no LLM/rootfs).

use std::time::Duration;

use serde_json::Value as Json;
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, run_workflow, stub_conversation, workflow_user};

const ONE_STEP_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: false
steps:
  - id: only
    kind: llm
    prompt: "do {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ only.output }}"
"#;

fn parse_sse_frame(frame: &str) -> Option<(String, String)> {
    let (mut ev, mut data) = (None, None);
    for line in frame.lines() {
        if let Some(r) = line.strip_prefix("event:") {
            ev = Some(r.trim().to_string());
        } else if let Some(r) = line.strip_prefix("data:") {
            data = Some(r.trim().to_string());
        }
    }
    Some((ev?, data?))
}

#[tokio::test]
async fn run_sse_opens_with_snapshot_and_delivers_terminal_status() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_sse_order").await;
    let wf = import_dev_workflow(&server, &user.token, "sse-order", ONE_STEP_YAML).await;
    let wf_id = wf["id"].as_str().unwrap().to_string();
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        serde_json::json!({
            "inputs": { "topic": "x" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "only": "mocked output" },
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    // Collect every SSE frame, IN ORDER, until the stream closes / 15s.
    let mut resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/events")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("subscribe");
    assert_eq!(resp.status(), 200);

    let mut frames: Vec<(String, Json)> = Vec::new();
    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    'outer: loop {
        let chunk = match tokio::time::timeout_at(deadline, resp.chunk()).await {
            Ok(Ok(Some(b))) => b,
            _ => break,
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find("\n\n") {
            let frame: String = buf.drain(..idx + 2).collect();
            if let Some((ev, data)) = parse_sse_frame(&frame) {
                let json: Json = serde_json::from_str(&data).unwrap_or(Json::Null);
                let is_terminal =
                    matches!(ev.as_str(), "runCompleted" | "runFailed" | "runCancelled");
                frames.push((ev, json));
                if is_terminal {
                    break 'outer; // terminal delivered
                }
            }
        }
    }

    assert!(!frames.is_empty(), "the SSE stream must deliver at least one frame");
    // Ordering: the stream opens with a snapshot.
    assert_eq!(frames[0].0, "snapshot", "the first SSE frame must be the snapshot; got {:?}", frames[0].0);
    // Completeness: the stream delivers the terminal `runCompleted` event.
    assert!(
        frames.iter().any(|(ev, _)| ev == "runCompleted"),
        "the stream must deliver the terminal 'runCompleted' event; frames={frames:?}"
    );
}
