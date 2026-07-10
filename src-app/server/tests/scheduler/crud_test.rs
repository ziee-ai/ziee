//! CRUD + owner-scope + permission + quota round-trips on scheduled tasks.

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Create a `prompt`-target recurring task body against `model_id`.
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
async fn create_get_list_update_delete() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "sched", &["scheduler::use"]).await;
    // Stub model (creates provider + model + grants this user access).
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Create.
    let resp = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&task_body(model_id, "Weekly digest"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let task: Value = resp.json().await.unwrap();
    let id = task["id"].as_str().unwrap().to_string();
    assert_eq!(task["name"], "Weekly digest");
    assert_eq!(task["user_id"], user.user_id);
    // A recurring task must have a computed next fire instant.
    assert!(task["next_run_at"].is_string(), "next_run_at should be set");

    // List.
    let list: Value = client()
        .get(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Get.
    let got = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), StatusCode::OK);

    // Update (rename).
    let updated = client()
        .put(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Renamed digest" }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated: Value = updated.json().await.unwrap();
    assert_eq!(updated["name"], "Renamed digest");

    // Delete.
    let deleted = client()
        .delete(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    // Gone.
    let gone = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn owner_scoped_cross_user_is_404() {
    let server = TestServer::start().await;
    let a = create_user_with_permissions(&server, "sched_a", &["scheduler::use"]).await;
    let b = create_user_with_permissions(&server, "sched_b", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &a.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", a.token))
        .json(&task_body(model_id, "A's task"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = task["id"].as_str().unwrap();

    // B cannot see A's task.
    let resp = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", b.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn permission_and_auth_gating() {
    let server = TestServer::start().await;
    // A user WITHOUT scheduler::use. `scheduler::use` is granted to the default
    // Users group (migration 142), so we must use `create_user_with_only_permissions`
    // (which removes the user from the default group) to get a genuinely
    // unauthorized user.
    let noperm = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "noperm",
        &["profile::read"],
    )
    .await;
    let resp = client()
        .get(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Unauthenticated.
    let resp = client()
        .get(server.api_url("/scheduled-tasks"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn quota_returns_422_at_cap() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "sched_admin",
        &["scheduler::admin::read", "scheduler::admin::manage"],
    )
    .await;
    // Lower the per-user cap to 1.
    let put = client()
        .put(server.api_url("/scheduler/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "max_active_tasks_per_user": 1,
            "min_interval_seconds": 300,
            "max_consecutive_failures": 5,
            "notification_retention_days": 30,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);

    let user = create_user_with_permissions(&server, "sched_q", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // First task: OK.
    let first = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&task_body(model_id, "T1"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    // Second task: over the cap → 422.
    let second = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&task_body(model_id, "T2"))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
