//! Regression (Phase-6 HIGH): a self-paced firing that FAILS must NOT run the
//! self-paced write-back.
//!
//! The write-back (`tick::fire_task`, the
//! `if trigger != "run_now" && matches!(SelfPaced) { .. }` block) re-arms the
//! next fire / self-completes, and on the plain self-paced path calls
//! `arm_self_paced(Disable, "completed")` when there is no proposal. A FAILED
//! firing has no proposal, so — before the fix — that unguarded write-back ran on
//! failure too, OVERWRITING the failure `paused_reason` (e.g. `'target_missing'`)
//! that `record_outcome` had just set with `'completed'` (masking the failure as
//! a success in the UI) and disabling the loop on the first blip (bypassing the
//! consecutive-failure cap). The fix gates the block on `outcome.success`.
//!
//! This drives a self-paced prompt task through a FAILED firing (its model is
//! removed, so `dispatch_prompt`'s fire-time model re-check fails → 404 →
//! `target_missing`, a terminal failure) via the real boot tick loop (sped up
//! with `SCHEDULER_TICK_MS`) and asserts the row settles with
//! `paused_reason == 'target_missing'` (NOT `'completed'`) and
//! `last_status == 'failed'`. It FAILS against the unfixed code (paused_reason is
//! masked to `'completed'`) and PASSES after the fix.

use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn get_task(server: &TestServer, token: &str, id: &str) -> Value {
    client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// GET the task row and return it once `pred` holds (or panic after ~30s).
async fn poll_task(
    server: &TestServer,
    token: &str,
    id: &str,
    label: &str,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..120 {
        let task = get_task(server, token, id).await;
        if pred(&task) {
            return task;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("self-paced task never reached the expected state: {label}");
}

/// A self-paced prompt task whose model is gone at fire time fails TERMINALLY
/// (`target_missing`) and must NOT self-complete: the failure `paused_reason`
/// stays `'target_missing'` (never masked to `'completed'`) and `last_status`
/// is `'failed'`.
#[tokio::test]
async fn self_paced_failed_firing_does_not_mask_failure_as_completed() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("SCHEDULER_TICK_MS".to_string(), "250".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "sp_fail", &["scheduler::use"]).await;
    // A valid stub model so the API create passes its model-access validation;
    // the model reference is removed below so the FIRING re-check fails.
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Create the task as a `once` task ~1h out so it is NOT yet claimable — this
    // eliminates any race: the task never fires while we set up the failure.
    let run_at = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Self-paced fail",
            "target_kind": "prompt",
            "prompt": "Work toward the goal.",
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
    let task_uuid = Uuid::parse_str(&id).unwrap();

    // Atomically flip the row into an ARMED, self-paced, model-less state: the API
    // won't create a self-paced task pointed at an inaccessible model (create
    // validates model access), so we do it directly. `model_id = NULL` makes
    // `dispatch_prompt`'s fire-time model re-check fail (404 → `target_missing`,
    // terminal). Because the task was never claimable until this single UPDATE
    // arms it, there is no window in which it could fire with the model present.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test database");
    sqlx::query!(
        r#"
        UPDATE scheduled_tasks
        SET schedule_kind = 'self_paced',
            model_id      = NULL,
            run_at        = NULL,
            next_run_at   = NOW()
        WHERE id = $1
        "#,
        task_uuid,
    )
    .execute(&pool)
    .await
    .expect("arm self-paced + remove model");
    pool.close().await;

    // The tick claims + fires it; the missing model makes the firing fail. Wait
    // until the failed firing is recorded on the row …
    poll_task(&server, &user.token, &id, "fired→failed", |t| {
        t["last_status"] == "failed"
    })
    .await;

    // … then settle so any (buggy) post-`record_outcome` write-back has definitely
    // run before we read the FINAL state. The whole `fire_task` completes within a
    // few ms of the fast-failing dispatch, so 1s is ample.
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let settled = get_task(&server, &user.token, &id).await;
    assert_eq!(
        settled["last_status"], "failed",
        "the firing failed, so last_status must be 'failed'"
    );
    // The core assertion: the real failure reason survives — it is NOT masked to
    // 'completed' by an unguarded self-paced write-back.
    assert_eq!(
        settled["paused_reason"], "target_missing",
        "a FAILED self-paced firing must keep its true failure reason, not be masked as 'completed' (paused_reason was {:?})",
        settled["paused_reason"]
    );
    assert_ne!(
        settled["paused_reason"], "completed",
        "a FAILED self-paced firing must never self-complete"
    );
    assert_eq!(
        settled["enabled"], false,
        "a terminally-failed task is disabled (by record_outcome)"
    );
}
