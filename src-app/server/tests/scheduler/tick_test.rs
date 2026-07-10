//! Tick-DRIVEN firing (not run-now): exercises the scheduled path —
//! claim_due_tasks → mark_fired (advance/disable) → dispatch → record_outcome →
//! notification — via the real boot tick loop, sped up with the debug
//! `SCHEDULER_TICK_MS` seam.

use std::time::Duration;

use chrono::Utc;
use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn wait_for_notification(server: &TestServer, token: &str) -> Value {
    for _ in 0..120 {
        let page: Value = client()
            .get(server.api_url("/notifications"))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if !page["items"].as_array().cloned().unwrap_or_default().is_empty() {
            return page;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("tick never fired the scheduled task (no notification within 60s)");
}

#[tokio::test]
async fn tick_fires_scheduled_once_prompt_and_disables_it() {
    // Fast tick so a due task fires within a couple seconds.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("SCHEDULER_TICK_MS".to_string(), "300".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "tick", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // A `once` task ~2s in the future — the TICK fires it (not run-now), so this
    // exercises mark_fired / record_outcome / the once-disable path.
    let run_at = (Utc::now() + chrono::Duration::seconds(2)).to_rfc3339();
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Tick fire",
            "target_kind": "prompt",
            "prompt": "Say hello.",
            "model_id": model_id,
            "schedule_kind": "once",
            "run_at": run_at,
            "timezone": "UTC",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = task["id"].as_str().unwrap().to_string();
    assert!(task["next_run_at"].is_string(), "once task should have next_run_at = run_at");
    assert_eq!(task["enabled"], true);

    // The tick fires it → a result notification lands.
    let page = wait_for_notification(&server, &user.token).await;
    assert_eq!(page["items"][0]["kind"], "scheduled_task_result");

    // Scheduled-path bookkeeping: the `once` task advanced to disabled + recorded
    // its status, and a run-history row exists.
    let refetched: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refetched["enabled"], false, "a once task disables after firing");
    assert!(refetched["next_run_at"].is_null(), "spent once task has no next run");
    assert_eq!(refetched["last_status"], "completed");

    let runs: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}/runs")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let runs = runs.as_array().unwrap();
    assert!(!runs.is_empty(), "the firing is recorded in run history");
    assert_eq!(runs[0]["trigger"], "schedule");
    assert_eq!(runs[0]["status"], "completed");
}

#[tokio::test]
async fn tick_run_now_does_not_disable_or_advance_a_recurring_task() {
    // run-now (off-schedule) must NOT touch the schedule bookkeeping (contract):
    // a recurring task run-now'd keeps enabled=true and its original next_run_at.
    let server = TestServer::start_with_options(TestServerOptions {
        // Slow tick so ONLY run-now fires within the test window (no scheduled fire).
        extra_env: vec![("SCHEDULER_TICK_MS".to_string(), "3600000".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "runnow", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Recurring",
            "target_kind": "prompt",
            "prompt": "Say hello.",
            "model_id": model_id,
            "schedule_kind": "recurring",
            "cron_expr": "0 9 * * 1",
            "timezone": "UTC",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = task["id"].as_str().unwrap().to_string();
    let next_before = task["next_run_at"].as_str().unwrap().to_string();

    // Run now, wait for its notification.
    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    wait_for_notification(&server, &user.token).await;

    // Bookkeeping untouched: still enabled, same next_run_at.
    let refetched: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refetched["enabled"], true, "run-now must not disable the task");
    assert_eq!(
        refetched["next_run_at"].as_str().unwrap(),
        next_before,
        "run-now must not advance next_run_at"
    );
    let _ = StatusCode::OK;
}
