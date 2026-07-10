//! Dry-run / test-fire (ITEM-34): `POST /api/scheduled-tasks/test-fire` runs the
//! target ONCE and returns the result inline with **all schedule side effects
//! suppressed** — no notification row, no `scheduled_tasks` row, no run history.
//! Gated `scheduler::use`. Mirrors the run-now path but proves the no-side-effect
//! contract (DEC / ITEM-34).

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::{
    create_user_with_only_permissions, create_user_with_permissions,
};

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn test_fire_returns_result_with_no_side_effects() {
    let server = TestServer::start().await;
    let user =
        create_user_with_permissions(&server, "dryrun", &["scheduler::use"]).await;
    let (_stub, model) =
        crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Test-fire an UNSAVED prompt config.
    let res = client()
        .post(server.api_url("/scheduled-tasks/test-fire"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "target_kind": "prompt",
            "prompt": "Say hello.",
            "model_id": model_id,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "test-fire should return 200 with the inline result"
    );
    let result: Value = res.json().await.unwrap();
    assert_eq!(result["ok"], true, "dry-run should succeed: {result}");
    assert!(
        result["text"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
        "dry-run returns the model output inline: {result}"
    );

    // NO durable side effects: no task row was created…
    let tasks: Value = client()
        .get(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        tasks.as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "test-fire must NOT persist a scheduled_tasks row"
    );

    // …and no notification landed in the inbox.
    let inbox: Value = client()
        .get(server.api_url("/notifications"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        inbox["items"].as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "test-fire must NOT write a notification"
    );
}

#[tokio::test]
async fn test_fire_requires_scheduler_use_permission() {
    let server = TestServer::start().await;
    // A user WITHOUT scheduler::use — `only_permissions` bypasses the Users-group
    // default grant so the user genuinely lacks the permission.
    let user = create_user_with_only_permissions(&server, "noperm", &[]).await;
    let res = client()
        .post(server.api_url("/scheduled-tasks/test-fire"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "target_kind": "prompt",
            "prompt": "hi",
            "model_id": "00000000-0000-0000-0000-000000000000",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "no scheduler::use → 403");
}
