//! Dispatch-path behaviors surfaced on the durable notification / task rows:
//!   * ITEM-29 — `notify_mode='silent'` writes the durable inbox row but marks it
//!     NON-interrupting (no toast/badge interrupt); `always` marks it interrupting.
//!   * ITEM-30 — a recurring `prompt` task owns ONE bound conversation: repeated
//!     firings append to the SAME conversation rather than spawning a fresh one.
//! Drives the real run-now → dispatch → notification path against a stub model.

use serde_json::{Value, json};
use std::time::Duration;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn create_and_run(server: &TestServer, token: &str, model_id: &str, notify_mode: &str) -> String {
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "Dispatch behavior",
            "target_kind": "prompt",
            "prompt": "Say hello.",
            "model_id": model_id,
            "schedule_kind": "recurring",
            "cron_expr": "0 9 * * 1",
            "timezone": "UTC",
            "notify_mode": notify_mode,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = task["id"].as_str().unwrap().to_string();
    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    id
}

async fn wait_notifications(server: &TestServer, token: &str, want: usize) -> Vec<Value> {
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
        let items = page["items"].as_array().cloned().unwrap_or_default();
        if items.len() >= want {
            return items;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("did not observe {want} notification(s) within 60s");
}

#[tokio::test]
async fn silent_notify_mode_writes_a_non_interrupting_row() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "silent", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    create_and_run(&server, &user.token, model_id, "silent").await;
    let items = wait_notifications(&server, &user.token, 1).await;

    // The durable row exists (so it's auditable in the inbox) but does NOT
    // interrupt (no toast/badge) — the low-signal triage channel.
    assert_eq!(items[0]["interrupt"], false, "silent → non-interrupting row");
}

#[tokio::test]
async fn always_notify_mode_writes_an_interrupting_row() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "loud", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    create_and_run(&server, &user.token, model_id, "always").await;
    let items = wait_notifications(&server, &user.token, 1).await;
    assert_eq!(items[0]["interrupt"], true, "always → interrupting row");
}

#[tokio::test]
async fn recurring_prompt_task_reuses_one_bound_conversation() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "bound", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Create a recurring prompt task and fire it twice via run-now.
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Bound",
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

    // Fire the FIRST run-now and wait for its notification — this guarantees the
    // task's `bound_conversation_id` is committed before the second firing reads
    // it (otherwise the two firings race to create two conversations).
    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    wait_notifications(&server, &user.token, 1).await;

    // Fire the SECOND run-now; it must reuse the now-committed bound conversation.
    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let items = wait_notifications(&server, &user.token, 2).await;
    // Both firings appended to the SAME conversation (bound), not two fresh ones.
    let convs: Vec<&str> = items
        .iter()
        .filter_map(|n| n["conversation_id"].as_str())
        .collect();
    assert_eq!(convs.len(), 2, "both firings link a conversation");
    assert_eq!(convs[0], convs[1], "both firings reuse ONE bound conversation");

    // The task row records the bound conversation id.
    let refetched: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        refetched["bound_conversation_id"].is_string(),
        "task pins its bound conversation id"
    );
}
