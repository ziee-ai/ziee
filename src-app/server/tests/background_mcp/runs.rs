//! ITEM-8 / ITEM-10 — REST to VIEW + MANAGE the acting user's background runs:
//!   - `GET  /api/background/runs`
//!   - `POST /api/background/runs/{run_id}/cancel`
//!
//! Covers the HTTP contract end-to-end against the real backend:
//!   - 401 (unauthenticated) + 403 (authenticated but lacking `background::use`)
//!     on BOTH endpoints;
//!   - list roundtrip: owner sees ONLY its own runs, newest-first, with the
//!     compact summary fields (label / has_result / job_kind / status);
//!   - pagination (page/per_page + total/total_pages);
//!   - `status` + `kind` filters (pushed to SQL);
//!   - cancel a RUNNING run → 200 + status flips to `cancelled`; a terminal run
//!     → 409; a foreign run → 404 (never leak).
//!
//! Runs are inserted directly (non-terminal / terminal `subagent` /
//! `sandbox_exec` `workflow_runs` rows) so the tests are deterministic — no
//! dependence on a live sub-agent turn.

use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{
    create_user_with_no_permissions, create_user_with_permissions, TestUser,
};

/// A user that can reach the background REST (`background::use`).
async fn bg_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["background::use"]).await
}

/// Insert a background `workflow_runs` row owned by `user_id`, returning its id.
/// Direct SQL (runtime query). `task` is stored under `inputs_json.task` (the
/// summary's `label` source); `with_output` sets `final_output_json` so the
/// summary's `has_result` flag is exercised.
async fn insert_bg_run(
    server: &TestServer,
    user_id: &str,
    kind: &str,
    status: &str,
    task: &str,
    with_output: bool,
) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let owner = Uuid::parse_str(user_id).unwrap();
    let inputs = json!({ "task": task });
    let output: Option<serde_json::Value> = if with_output {
        Some(json!({ "final_text": "done" }))
    } else {
        None
    };
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workflow_runs (job_kind, user_id, status, inputs_json, final_output_json) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(kind)
    .bind(owner)
    .bind(status)
    .bind(inputs)
    .bind(output)
    .fetch_one(&pool)
    .await
    .expect("insert background run")
}

/// Read a run's status directly from the DB (deterministic cancel assertion).
async fn db_status(server: &TestServer, run_id: Uuid) -> String {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query_scalar::<_, String>("SELECT status FROM workflow_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("read run status")
}

fn list_url(server: &TestServer) -> String {
    server.api_url("/background/runs")
}
fn cancel_url(server: &TestServer, run_id: Uuid) -> String {
    server.api_url(&format!("/background/runs/{run_id}/cancel"))
}
fn detail_url(server: &TestServer, run_id: Uuid) -> String {
    server.api_url(&format!("/background/runs/{run_id}"))
}

#[tokio::test]
async fn list_and_cancel_require_auth() {
    let server = TestServer::start().await;
    let user = bg_user(&server, "bg_runs_auth").await;
    let run_id = insert_bg_run(&server, &user.user_id, "subagent", "running", "t", false).await;
    let client = reqwest::Client::new();

    // No Authorization header → 401 on both.
    let list = client.get(list_url(&server)).send().await.unwrap();
    assert_eq!(list.status(), 401, "unauthenticated list must be 401");

    let cancel = client.post(cancel_url(&server, run_id)).send().await.unwrap();
    assert_eq!(cancel.status(), 401, "unauthenticated cancel must be 401");
}

#[tokio::test]
async fn list_and_cancel_require_background_use_permission() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_owner_perm").await;
    let run_id = insert_bg_run(&server, &owner.user_id, "subagent", "running", "t", false).await;

    // Authenticated but WITHOUT `background::use` → 403 (the perm gate fires
    // before ownership resolution, so it's 403 not 404). NOTE: `background::use`
    // is granted to the default Users group (migration 202607191000), which every
    // registered user auto-joins — so `create_user_with_no_permissions` (strips
    // ALL group membership) is required to genuinely LACK the permission.
    let noperm = create_user_with_no_permissions(&server, "bg_runs_noperm").await;
    let client = reqwest::Client::new();

    let list = client
        .get(list_url(&server))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 403, "missing background::use must 403 on list");

    let cancel = client
        .post(cancel_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(cancel.status(), 403, "missing background::use must 403 on cancel");
}

#[tokio::test]
async fn list_returns_only_owner_runs_newest_first() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_list_owner").await;
    let other = bg_user(&server, "bg_runs_list_other").await;
    let client = reqwest::Client::new();

    // Two runs for the owner (the 2nd is inserted later → newer), one for other.
    let _r1 = insert_bg_run(&server, &owner.user_id, "subagent", "completed", "first task", true).await;
    let r2 = insert_bg_run(&server, &owner.user_id, "subagent", "running", "second task", false).await;
    let _foreign = insert_bg_run(&server, &other.user_id, "subagent", "running", "not yours", false).await;

    let resp = client
        .get(list_url(&server))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["total"], 2, "owner has exactly 2 background runs (no leak)");
    let runs = body["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);

    // Newest-first: r2 (running / "second task") leads.
    assert_eq!(runs[0]["id"], r2.to_string());
    assert_eq!(runs[0]["status"], "running");
    assert_eq!(runs[0]["job_kind"], "subagent");
    assert_eq!(runs[0]["label"], "second task", "label derives from spec.task");
    assert_eq!(runs[0]["has_result"], false, "a running run has no result yet");
    // The completed run carries a result + label.
    assert_eq!(runs[1]["status"], "completed");
    assert_eq!(runs[1]["label"], "first task");
    assert_eq!(runs[1]["has_result"], true, "a run with final_output_json has a result");

    // The heavy blob is NOT projected into the summary.
    assert!(runs[1].get("final_output_json").is_none(), "summary excludes final_output_json");
}

#[tokio::test]
async fn list_paginates() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_paginate").await;
    let client = reqwest::Client::new();

    for i in 0..3 {
        insert_bg_run(&server, &owner.user_id, "subagent", "running", &format!("task {i}"), false).await;
    }

    // page 1, per_page 2 → 2 runs, total 3, total_pages 2.
    let p1: serde_json::Value = client
        .get(format!("{}?page=1&per_page=2", list_url(&server)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(p1["total"], 3);
    assert_eq!(p1["page"], 1);
    assert_eq!(p1["per_page"], 2);
    assert_eq!(p1["total_pages"], 2);
    assert_eq!(p1["runs"].as_array().unwrap().len(), 2);

    // page 2 → the remaining 1.
    let p2: serde_json::Value = client
        .get(format!("{}?page=2&per_page=2", list_url(&server)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(p2["runs"].as_array().unwrap().len(), 1, "page 2 has the remaining run");
}

#[tokio::test]
async fn list_filters_by_status_and_kind() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_filter").await;
    let client = reqwest::Client::new();

    insert_bg_run(&server, &owner.user_id, "subagent", "running", "a", false).await;
    insert_bg_run(&server, &owner.user_id, "subagent", "completed", "b", true).await;
    insert_bg_run(&server, &owner.user_id, "sandbox_exec", "running", "c", false).await;

    // status=completed → only the completed subagent.
    let by_status: serde_json::Value = client
        .get(format!("{}?status=completed", list_url(&server)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(by_status["total"], 1, "one completed run");
    assert_eq!(by_status["runs"][0]["status"], "completed");

    // kind=sandbox_exec → only the sandbox_exec run.
    let by_kind: serde_json::Value = client
        .get(format!("{}?kind=sandbox_exec", list_url(&server)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(by_kind["total"], 1, "one sandbox_exec run");
    assert_eq!(by_kind["runs"][0]["job_kind"], "sandbox_exec");
}

#[tokio::test]
async fn cancel_running_run_flips_to_cancelled() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_cancel_ok").await;
    let run_id = insert_bg_run(&server, &owner.user_id, "subagent", "running", "cancel me", false).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(cancel_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "owner cancel of a running run must be 200");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "cancelled");
    assert_eq!(body["run_id"], run_id.to_string());

    // The DB row is now terminal `cancelled` (the CAS flip is authoritative even
    // with no resident in-memory runner for this directly-inserted row).
    assert_eq!(db_status(&server, run_id).await, "cancelled");
}

#[tokio::test]
async fn cancel_terminal_run_is_409() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_cancel_terminal").await;
    let run_id = insert_bg_run(&server, &owner.user_id, "subagent", "completed", "already done", true).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(cancel_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409, "cancelling a terminal run must be 409");
    // The completed run is untouched.
    assert_eq!(db_status(&server, run_id).await, "completed");
}

#[tokio::test]
async fn cancel_foreign_run_is_404() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_runs_cancel_owner").await;
    let other = bg_user(&server, "bg_runs_cancel_other").await;
    let run_id = insert_bg_run(&server, &owner.user_id, "subagent", "running", "not yours", false).await;
    let client = reqwest::Client::new();

    // User B (has background::use, but not this run) → 404 (never leak).
    let resp = client
        .post(cancel_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "cross-user cancel must be 404 (never leak)");
    // The run is untouched — still running.
    assert_eq!(db_status(&server, run_id).await, "running");
}

// ── GET /api/background/runs/{run_id} — single-run detail (incl. result) ─────

#[tokio::test]
async fn detail_requires_auth() {
    let server = TestServer::start().await;
    let user = bg_user(&server, "bg_detail_auth").await;
    let run_id = insert_bg_run(&server, &user.user_id, "subagent", "completed", "t", true).await;
    let client = reqwest::Client::new();

    // No Authorization header → 401.
    let resp = client.get(detail_url(&server, run_id)).send().await.unwrap();
    assert_eq!(resp.status(), 401, "unauthenticated detail must be 401");
}

#[tokio::test]
async fn detail_requires_background_use_permission() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_detail_owner_perm").await;
    let run_id = insert_bg_run(&server, &owner.user_id, "subagent", "completed", "t", true).await;

    // Authenticated but WITHOUT `background::use` → 403 (the perm gate fires before
    // ownership resolution). `background::use` is granted to the default Users group,
    // so `create_user_with_no_permissions` (strips ALL group membership) is required.
    let noperm = create_user_with_no_permissions(&server, "bg_detail_noperm").await;
    let client = reqwest::Client::new();

    let resp = client
        .get(detail_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "missing background::use must 403 on detail");
}

#[tokio::test]
async fn detail_returns_full_run_including_final_output() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_detail_owner").await;
    let run_id =
        insert_bg_run(&server, &owner.user_id, "subagent", "completed", "detail task", true).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(detail_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "owner detail of its own run must be 200");
    let body: serde_json::Value = resp.json().await.unwrap();

    // Identity / state / timing fields (mirror the summary).
    assert_eq!(body["id"], run_id.to_string());
    assert_eq!(body["job_kind"], "subagent");
    assert_eq!(body["status"], "completed");
    assert_eq!(body["label"], "detail task", "label derives from spec.task");
    assert_eq!(body["has_result"], true, "a run with final_output_json has a result");
    assert!(body.get("created_at").is_some(), "timings present");
    assert!(body.get("updated_at").is_some(), "timings present");

    // The DISTINGUISHING field: the full result body IS projected here (unlike the
    // compact list). `insert_bg_run(..with_output=true)` writes `{final_text:"done"}`.
    assert!(
        body.get("final_output_json").is_some() && !body["final_output_json"].is_null(),
        "detail includes final_output_json result body"
    );
    assert_eq!(
        body["final_output_json"]["final_text"], "done",
        "the collected result body round-trips verbatim"
    );
}

#[tokio::test]
async fn detail_of_running_run_has_null_result() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_detail_running").await;
    // A still-running run with no result yet.
    let run_id =
        insert_bg_run(&server, &owner.user_id, "subagent", "running", "in progress", false).await;
    let client = reqwest::Client::new();

    let body: serde_json::Value = client
        .get(detail_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["status"], "running");
    assert_eq!(body["has_result"], false, "a running run has no result");
    assert!(body["final_output_json"].is_null(), "no result body until terminal");
}

#[tokio::test]
async fn detail_of_foreign_run_is_404() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_detail_foreign_owner").await;
    let other = bg_user(&server, "bg_detail_foreign_other").await;
    let run_id =
        insert_bg_run(&server, &owner.user_id, "subagent", "completed", "not yours", true).await;
    let client = reqwest::Client::new();

    // User B (has background::use, but not this run) → 404 (never leak).
    let resp = client
        .get(detail_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "cross-user detail must be 404 (never leak)");
}
