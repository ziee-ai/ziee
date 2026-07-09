//! Realtime sync emission for the notification inbox (ITEM-16): a background
//! firing that lands a notification fans a `notification` create to the OWNER
//! only — a second user on their own device sees silence (cross-user
//! isolation, the positive-control pattern). Mirrors
//! `tests/memory/sync_emit_test.rs`.

use std::time::Duration;

use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(20);
const SILENCE: Duration = Duration::from_secs(2);

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn run_now_notification_fans_to_owner_only() {
    let server = TestServer::start().await;
    let owner =
        create_user_with_permissions(&server, "notifowner", &["scheduler::use"]).await;
    let outsider =
        create_user_with_permissions(&server, "notifout", &["scheduler::use"]).await;
    let (_stub, model) =
        crate::chat::helpers::create_stub_model(&server, &owner.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Owner creates a task.
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({
            "name": "Ping",
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

    // Probes opened AFTER create so the create-emit doesn't pollute the window.
    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    // Run now → a result notification is written for the owner.
    let res = client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "run-now status {}", res.status());

    // Owner's device receives the `notification` create; the outsider does not.
    owner_probe
        .expect_event("notification", "create", EVENT_TIMEOUT)
        .await;
    outsider_probe.expect_silence(SILENCE).await;
}
