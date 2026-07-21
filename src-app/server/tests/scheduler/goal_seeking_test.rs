//! TEST-122 (ITEM-24 / DEC-61/62/63) — the GOAL-SEEKING verification loop,
//! end-to-end through the real boot tick loop.
//!
//! A goal-seeking task is a `self_paced` prompt task carrying a natural-language
//! `completion_condition`. After each fired turn an isolated evaluator judges the
//! result against the condition; `done` self-stops the loop, `not_done` re-arms
//! another turn. The evaluator model call is made deterministic via the
//! debug-only `SCHEDULER_GOAL_EVAL_FORCE` seam (mirrors `SCHEDULER_TICK_MS`), so
//! this test proves the wiring without a live LLM. The exhaustive cap/horizon
//! branch logic is unit-covered in `scheduler::goal_eval::tests::decide_*` (fast,
//! deterministic; a real cap run would need N × the 300s min-interval).

use std::time::Duration;

use serde_json::{Value, json};

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// GET the task row and return it once `pred` holds (or panic after ~30s).
async fn poll_task(server: &TestServer, token: &str, id: &str, label: &str, pred: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..120 {
        let task: Value = client()
            .get(server.api_url(&format!("/scheduled-tasks/{id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if pred(&task) {
            return task;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("goal-seeking task never reached the expected state: {label}");
}

async fn create_goal_task(server: &TestServer, token: &str, model_id: &str) -> String {
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "Goal seek",
            "target_kind": "prompt",
            "prompt": "Work toward the goal.",
            "model_id": model_id,
            "schedule_kind": "self_paced",
            "timezone": "UTC",
            "completion_condition": "the analysis is complete and all values are filled in",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(task["completion_condition"], "the analysis is complete and all values are filled in");
    task["id"].as_str().unwrap().to_string()
}

/// Evaluator forced to `done` → the goal task fires one turn then SELF-STOPS,
/// reusing the self-paced Disable path (enabled=false, paused_reason='completed').
#[tokio::test]
async fn goal_task_self_stops_completed_when_evaluator_says_done() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("SCHEDULER_TICK_MS".to_string(), "300".to_string()),
            ("SCHEDULER_GOAL_EVAL_FORCE".to_string(), "done".to_string()),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "goal_done", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let id = create_goal_task(&server, &user.token, model_id).await;

    // The tick fires it; the forced-`done` evaluator self-stops the loop.
    let task = poll_task(&server, &user.token, &id, "done→completed", |t| {
        t["paused_reason"] == "completed"
    })
    .await;
    assert_eq!(task["enabled"], false, "a completed goal task is disabled");
    assert!(task["next_run_at"].is_null(), "a completed goal task has no next run");
    assert_eq!(task["paused_reason"], "completed");
}

/// Evaluator forced to `not_done` → the goal task fires a turn and RE-ARMS another
/// (enabled stays true, next_run_at is re-set to a future instant) — the loop
/// keeps working rather than self-completing.
#[tokio::test]
async fn goal_task_rearms_when_evaluator_says_not_done() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("SCHEDULER_TICK_MS".to_string(), "300".to_string()),
            ("SCHEDULER_GOAL_EVAL_FORCE".to_string(), "not_done".to_string()),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "goal_notdone", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let id = create_goal_task(&server, &user.token, model_id).await;

    // After the turn fires: last_status='completed' (the turn ran) AND next_run_at
    // is re-armed (non-null) AND the task is still enabled → the loop continues.
    // (A confirmed `done` would instead disable + null next_run_at.)
    let task = poll_task(&server, &user.token, &id, "not_done→re-armed", |t| {
        t["last_status"] == "completed"
            && t["enabled"] == true
            && t["next_run_at"].is_string()
    })
    .await;
    assert!(task["paused_reason"].is_null(), "a re-armed goal task is not paused/completed");
}
