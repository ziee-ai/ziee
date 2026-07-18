//! End-to-end firing → notification inbox, plus inbox CRUD.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_only_permissions, create_user_with_permissions};

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Create a `prompt` task, then run it now; returns the task id.
async fn create_and_run_now(server: &TestServer, token: &str, model_id: &str) -> String {
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "Fire test",
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

    let run = client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(run.status(), StatusCode::ACCEPTED);
    id
}

/// Poll the inbox until at least one notification appears (or time out).
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
        let items = page["items"].as_array().cloned().unwrap_or_default();
        if !items.is_empty() {
            return page;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("no notification appeared within 60s after run-now");
}

#[tokio::test]
async fn run_now_prompt_produces_a_notification() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "notif", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    create_and_run_now(&server, &user.token, model_id).await;
    let page = wait_for_notification(&server, &user.token).await;

    let items = page["items"].as_array().unwrap();
    let n = &items[0];
    // The firing must SUCCEED (a real scheduled_task_result, not a failure) and
    // link the conversation the prompt turn created.
    assert_eq!(
        n["kind"], "scheduled_task_result",
        "expected a success notification, got {n:?}"
    );
    // R2: kind-specific ids ride the `payload` jsonb column, not top-level FKs.
    assert!(
        n["payload"]["conversation_id"].is_string(),
        "should link the conversation via payload"
    );
    assert!(n["read_at"].is_null(), "should arrive unread");
    assert_eq!(page["unread"], 1);
}

#[tokio::test]
async fn inbox_crud() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "notif_crud", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    create_and_run_now(&server, &user.token, model_id).await;
    let page = wait_for_notification(&server, &user.token).await;
    let id = page["items"][0]["id"].as_str().unwrap().to_string();

    // Unread count = 1.
    let uc: Value = client()
        .get(server.api_url("/notifications/unread-count"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(uc["unread"], 1);

    // Mark read → unread goes to 0.
    let read: Value = client()
        .post(server.api_url(&format!("/notifications/{id}/read")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(read["unread"], 0);

    // Delete → gone.
    let del = client()
        .delete(server.api_url(&format!("/notifications/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    let gone = client()
        .get(server.api_url(&format!("/notifications/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn owner_scope_and_gating() {
    let server = TestServer::start().await;
    let a = create_user_with_permissions(&server, "notif_a", &["scheduler::use"]).await;
    let b = create_user_with_permissions(&server, "notif_b", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &a.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    create_and_run_now(&server, &a.token, model_id).await;
    let page = wait_for_notification(&server, &a.token).await;
    let id = page["items"][0]["id"].as_str().unwrap().to_string();

    // B cannot see A's notification.
    let cross = client()
        .get(server.api_url(&format!("/notifications/{id}")))
        .header("Authorization", format!("Bearer {}", b.token))
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), StatusCode::NOT_FOUND);

    // A user without notifications::read → 403.
    let noperm =
        create_user_with_only_permissions(&server, "notif_noperm", &["profile::read"]).await;
    let forbidden = client()
        .get(server.api_url("/notifications"))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    // Unauthenticated → 401.
    let unauth = client()
        .get(server.api_url("/notifications"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);
}
