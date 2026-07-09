//! Realtime sync emission for the scheduler surface (ITEM-13 / ITEM-18):
//! task create/update/delete fan a `scheduled_task` event to the OWNER only
//! (owner audience, cross-user isolation), and an admin-settings update fans a
//! `scheduler_admin_settings` event to admin-perm holders. Mirrors
//! `tests/summarization/sync_emit_test.rs`.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
const SILENCE: Duration = Duration::from_millis(800);

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn task_body(model_id: &str, name: &str) -> Value {
    json!({
        "name": name,
        "target_kind": "prompt",
        "prompt": "Summarize today's news.",
        "model_id": model_id,
        "schedule_kind": "recurring",
        "cron_expr": "0 9 * * 1",
        "timezone": "UTC",
    })
}

#[tokio::test]
async fn task_mutations_emit_scheduled_task_to_owner_only() {
    let server = TestServer::start().await;
    let owner =
        create_user_with_permissions(&server, "owner", &["scheduler::use"]).await;
    let outsider =
        create_user_with_permissions(&server, "outsider", &["scheduler::use"]).await;
    let (_stub, model) =
        crate::chat::helpers::create_stub_model(&server, &owner.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    // Create → owner sees a `scheduled_task` create; the outsider sees nothing.
    let created: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&task_body(model_id, "Digest"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    owner_probe
        .expect_event("scheduled_task", "create", EVENT_TIMEOUT)
        .await;
    outsider_probe.expect_silence(SILENCE).await;

    // Update → owner sees an update.
    let ok = client()
        .put(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({ "name": "Digest v2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    owner_probe
        .expect_event("scheduled_task", "update", EVENT_TIMEOUT)
        .await;
    outsider_probe.expect_silence(SILENCE).await;

    // Delete → owner sees a delete.
    let del = client()
        .delete(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
    owner_probe
        .expect_event("scheduled_task", "delete", EVENT_TIMEOUT)
        .await;
    outsider_probe.expect_silence(SILENCE).await;
}

#[tokio::test]
async fn admin_settings_update_emits_to_admin_holders() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "schedadmin",
        &["scheduler::admin::read", "scheduler::admin::manage"],
    )
    .await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;

    let res = client()
        .put(server.api_url("/scheduler/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "max_active_tasks_per_user": 25,
            "min_interval_seconds": 300,
            "max_consecutive_failures": 5,
            "notification_retention_days": 30,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "admin settings update: {}",
        res.text().await.unwrap_or_default()
    );

    let f = admin_probe
        .expect_event("scheduler_admin_settings", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        f.id, "00000000-0000-0000-0000-000000000000",
        "singleton nil id"
    );
}
