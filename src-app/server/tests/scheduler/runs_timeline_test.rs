//! Round 2 (ITEM-40/41): per-run `result_preview` + `change_summary_json` and the
//! PAGED run-history endpoint that drive the runs timeline.

use serde_json::{Value, json};
use std::time::Duration;

use reqwest::StatusCode;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn create_prompt_task(server: &TestServer, token: &str, model_id: &str, name: &str) -> String {
    let task: Value = client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
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
    task["id"].as_str().unwrap().to_string()
}

async fn run_now(server: &TestServer, token: &str, id: &str) {
    client()
        .post(server.api_url(&format!("/scheduled-tasks/{id}/run-now")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
}

/// Poll the paged runs endpoint until `total >= want`, returning the page-1 body.
async fn wait_for_runs(server: &TestServer, token: &str, id: &str, want: i64) -> Value {
    for _ in 0..160 {
        let body: Value = client()
            .get(server.api_url(&format!("/scheduled-tasks/{id}/runs")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if body["total"].as_i64().unwrap_or(0) >= want {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("did not reach {want} runs within 80s");
}

// TEST-40 (ITEM-40): a firing records a non-empty result_preview + a change_summary.
#[tokio::test]
async fn run_records_preview_and_change_summary() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tl", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let id = create_prompt_task(&server, &user.token, model_id, "preview").await;
    run_now(&server, &user.token, &id).await;
    let body = wait_for_runs(&server, &user.token, &id, 1).await;

    let run = &body["runs"][0];
    let preview = run["result_preview"].as_str();
    assert!(
        preview.is_some() && !preview.unwrap().is_empty(),
        "the firing records a non-empty result_preview, got {:?}",
        run["result_preview"]
    );
    let cs = &run["change_summary_json"];
    assert!(cs.is_object(), "change_summary_json is an object: {cs}");
    assert!(cs.get("changed").is_some(), "change summary carries `changed`");
    assert!(cs.get("new_count").is_some(), "change summary carries `new_count`");
}

// TEST-42 (ITEM-41): the runs endpoint pages — per_page bounds the slice, total
// counts all runs, and page 2 returns a different slice.
#[tokio::test]
async fn runs_endpoint_paginates() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "pg", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let id = create_prompt_task(&server, &user.token, model_id, "paged").await;
    // Two sequential firings → two run rows.
    run_now(&server, &user.token, &id).await;
    wait_for_runs(&server, &user.token, &id, 1).await;
    run_now(&server, &user.token, &id).await;
    wait_for_runs(&server, &user.token, &id, 2).await;

    let page1: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}/runs?page=1&per_page=1")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page1["total"].as_i64().unwrap(), 2, "total counts all runs");
    assert_eq!(page1["per_page"].as_i64().unwrap(), 1);
    assert_eq!(page1["runs"].as_array().unwrap().len(), 1, "per_page bounds the page");
    let id1 = page1["runs"][0]["id"].as_str().unwrap().to_string();

    let page2: Value = client()
        .get(server.api_url(&format!("/scheduled-tasks/{id}/runs?page=2&per_page=1")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page2["runs"].as_array().unwrap().len(), 1, "page 2 has the other run");
    let id2 = page2["runs"][0]["id"].as_str().unwrap();
    assert_ne!(id1, id2, "page 2 is a different run than page 1");
}

// TEST-42 (ITEM-41, guard): an out-of-range `page` does not error (empty page, real total).
#[tokio::test]
async fn runs_endpoint_huge_page_is_safe() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "pgbig", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let id = create_prompt_task(&server, &user.token, model_id, "bigpage").await;
    run_now(&server, &user.token, &id).await;
    wait_for_runs(&server, &user.token, &id, 1).await;

    // A very large page must not 500/panic (i64 offset overflow guard).
    let res = client()
        .get(server.api_url(&format!(
            "/scheduled-tasks/{id}/runs?page=5000000000000000000&per_page=200"
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "a huge page is clamped, not a 500");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["total"].as_i64().unwrap(), 1, "total is still correct");
    assert_eq!(body["runs"].as_array().unwrap().len(), 0, "an out-of-range page is empty");
}
