//! TEST-1 (acceptance, INV-1) — a background run's agent-loop activity is
//! persisted and read back via `GET /api/background/runs/{id}`.
//!
//! Drives the REAL persist path deterministically (no live LLM): a background
//! `workflow_runs` row is inserted, then the SAME `PersistingActivitySink` the
//! detached background loop uses (keyed to the run's own id + step `"agent"`)
//! emits the exact `AgentEvent` sequence a completed loop emits — a thinking
//! block, a tool call, and the final assistant message. The endpoint then returns
//! an `activity[]` carrying thinking + tool_call + message entries, owner-scoped.

use agent_core::{AgentEvent, EventSink};
use ai_providers::{ChatMessage, ContentBlock, Role};
use serde_json::{Value, json};
use uuid::Uuid;
use ziee::workflow::PersistingActivitySink;

use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

async fn bg_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["background::use"]).await
}

/// A completed background sub-agent turn: thinking → tool call → final message.
fn completed_turn_event() -> AgentEvent {
    AgentEvent::Message(ChatMessage::with_blocks(
        Role::Assistant,
        vec![
            ContentBlock::Thinking {
                thinking: "planning the background task".into(),
                signature: None,
            },
            ContentBlock::ToolUse {
                id: "t1".into(),
                name: "search".into(),
                input: json!({ "q": "background topic" }),
            },
            ContentBlock::Text {
                text: "background task complete".into(),
            },
        ],
    ))
}

#[tokio::test]
async fn background_run_transcript_is_persisted_and_readable() {
    let server = TestServer::start().await;
    let user = bg_user(&server, "bg_transcript").await;
    let owner = Uuid::parse_str(&user.user_id).unwrap();
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    // A background run row (its own workflow_runs row, as `insert_background_run`
    // makes) — the sink's persist target.
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (job_kind, user_id, status, inputs_json) \
         VALUES ('subagent', $1, 'completed', $2) RETURNING id",
    )
    .bind(owner)
    .bind(json!({ "task": "do the background thing" }))
    .fetch_one(&pool)
    .await
    .expect("insert background run");

    // Emit the completed-turn activity through the REAL persisting sink.
    let sink = PersistingActivitySink::new(pool.clone(), run_id, "agent");
    sink.emit(completed_turn_event()).await;

    // Read back via the owner-scoped endpoint.
    let client = reqwest::Client::new();
    let resp = client
        .get(server.api_url(&format!("/background/runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "owner reads its own background run");
    let body: Value = resp.json().await.unwrap();

    let activity = body["activity"].as_array().expect("activity[] present");
    let kinds: Vec<&str> = activity
        .iter()
        .map(|a| a["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"thinking"),
        "transcript has a thinking entry: {kinds:?}"
    );
    assert!(
        kinds.contains(&"tool_call"),
        "transcript has a tool_call entry: {kinds:?}"
    );
    assert!(
        kinds.contains(&"message"),
        "transcript has the final message entry: {kinds:?}"
    );
    // Ordered by seq (thinking then tool then message).
    let seqs: Vec<i64> = activity
        .iter()
        .map(|a| a["seq"].as_i64().unwrap())
        .collect();
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "entries are seq-ascending: {seqs:?}"
    );
}

#[tokio::test]
async fn background_run_transcript_is_owner_scoped_404() {
    // INV-4 partial: a foreign user cannot read another user's background transcript.
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_ts_owner").await;
    let other = bg_user(&server, "bg_ts_other").await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (job_kind, user_id, status, inputs_json) \
         VALUES ('subagent', $1, 'completed', '{}'::jsonb) RETURNING id",
    )
    .bind(Uuid::parse_str(&owner.user_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(server.api_url(&format!("/background/runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        404,
        "a foreign background run is 404, never leaked"
    );
}
