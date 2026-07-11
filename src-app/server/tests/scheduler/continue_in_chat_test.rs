//! Continue-in-chat (ITEM-32): `POST /api/scheduled-tasks/runs/{run_id}/continue`
//! opens a NEW conversation seeded with the run's context so the user can keep
//! chatting about a background result. Owner-scoped.

use serde_json::{Value, json};
use std::time::Duration;

use reqwest::StatusCode;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Create a recurring prompt task, run it now, and return (task_id, run_id) once
/// a run row exists.
async fn task_with_a_run(server: &TestServer, token: &str, model_id: &str) -> (String, String) {
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "Continue me",
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

    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    for _ in 0..120 {
        let runs: Value = client()
            .get(server.api_url(&format!("/scheduled-tasks/{id}/runs")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(first) = runs["runs"].as_array().and_then(|a| a.first()) {
            return (id, first["id"].as_str().unwrap().to_string());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("no run recorded within 60s");
}

#[tokio::test]
async fn continue_creates_a_seeded_conversation() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cont", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let (_task_id, run_id) = task_with_a_run(&server, &user.token, model_id).await;

    // Continue → a fresh conversation is created and returned.
    let res = client()
        .post(server.api_url(&format!("/scheduled-tasks/runs/{run_id}/continue")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "continue should create a conversation"
    );
    let body: Value = res.json().await.unwrap();
    let conv_id = body["conversation_id"].as_str().unwrap();

    // The returned conversation is real + owned by the user (GET succeeds).
    let conv = client()
        .get(server.api_url(&format!("/conversations/{conv_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        conv.status(),
        StatusCode::OK,
        "the seeded conversation is fetchable by its owner"
    );
}

// TEST-44 (ITEM-42): continuing a PROMPT run seeds the new conversation with the
// run's REAL assistant text (carried as a synthesized assistant turn, DEC-23) —
// not a status-only placeholder.
#[tokio::test]
async fn continue_prompt_run_seeds_real_assistant_text() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "seed", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let (_task_id, run_id) = task_with_a_run(&server, &user.token, model_id).await;

    let body: Value = client()
        .post(server.api_url(&format!("/scheduled-tasks/runs/{run_id}/continue")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let conv_id = body["conversation_id"].as_str().unwrap();
    let conv_uuid = uuid::Uuid::parse_str(conv_id).unwrap();

    let history =
        crate::chat::helpers::get_conversation_history(&server, &user.token, conv_uuid).await;
    let dump = history.to_string();
    // The stub model's canned reply is "Hello from stub"; it must be carried into
    // the seed (real result), and an assistant-role message must be present.
    assert!(
        dump.contains("Hello from stub"),
        "the seed carries the run's real assistant text, got: {dump}"
    );
    assert!(dump.contains("\"assistant\""), "the result rides an assistant turn");
}

// TEST-46 (ITEM-43): the series follow-up seeds a conversation and is owner-scoped.
#[tokio::test]
async fn continue_series_seeds_and_is_owner_scoped() {
    let server = TestServer::start().await;
    let owner = create_user_with_permissions(&server, "sowner", &["scheduler::use"]).await;
    let other = create_user_with_permissions(&server, "sother", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &owner.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let (task_id, _run_id) = task_with_a_run(&server, &owner.token, model_id).await;

    // Owner: series continue → 201 + a fetchable conversation.
    let res = client()
        .post(server.api_url(&format!("/scheduled-tasks/{task_id}/continue-series?limit=5")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED, "series continue creates a conversation");
    let conv_id = res.json::<Value>().await.unwrap()["conversation_id"]
        .as_str()
        .unwrap()
        .to_string();
    let conv = client()
        .get(server.api_url(&format!("/conversations/{conv_id}")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(conv.status(), StatusCode::OK, "owner can open the seeded series conversation");

    // A different user cannot seed a series from someone else's task.
    let foreign = client()
        .post(server.api_url(&format!("/scheduled-tasks/{task_id}/continue-series?limit=5")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND, "cross-user series → 404");
}

#[tokio::test]
async fn continue_run_owner_scoped_404() {
    let server = TestServer::start().await;
    let owner = create_user_with_permissions(&server, "owner", &["scheduler::use"]).await;
    let other = create_user_with_permissions(&server, "other", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &owner.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let (_task_id, run_id) = task_with_a_run(&server, &owner.token, model_id).await;

    // A different user cannot continue someone else's run.
    let res = client()
        .post(server.api_url(&format!("/scheduled-tasks/runs/{run_id}/continue")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "cross-user run → 404"
    );
}
