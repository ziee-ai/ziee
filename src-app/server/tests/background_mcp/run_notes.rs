//! ITEM-25 — REST for STEERING a running background run:
//! `POST/GET /api/background/runs/{run_id}/notes`.
//!
//! Covers the HTTP contract end-to-end against the real backend:
//!   - 401 (unauthenticated) + 403 (authenticated but lacking `background::use`);
//!   - owner enqueue → list roundtrip (the note is persisted + returned pending);
//!   - cross-user isolation (user B → user A's run yields 404, never leaks).
//!
//! The run is inserted directly (a non-terminal `subagent` `workflow_runs` row)
//! so the test is deterministic — no dependence on a live sub-agent turn.

use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, TestUser};

/// A user that can reach the background steering REST (`background::use`).
async fn bg_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(server, name, &["background::use"]).await
}

/// Insert a NON-terminal `subagent` background run owned by `user_id`, returning
/// its id. Direct SQL (runtime query) — the run's lifecycle is irrelevant here;
/// we only need a steerable owner-scoped row.
async fn insert_subagent_run(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let owner = Uuid::parse_str(user_id).unwrap();
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workflow_runs (job_kind, user_id, status) \
         VALUES ('subagent', $1, 'running') RETURNING id",
    )
    .bind(owner)
    .fetch_one(&pool)
    .await
    .expect("insert subagent run")
}

fn notes_url(server: &TestServer, run_id: Uuid) -> String {
    server.api_url(&format!("/background/runs/{run_id}/notes"))
}

#[tokio::test]
async fn post_run_note_requires_auth() {
    let server = TestServer::start().await;
    let user = bg_user(&server, "bg_notes_auth").await;
    let run_id = insert_subagent_run(&server, &user.user_id).await;

    // No Authorization header → 401.
    let resp = reqwest::Client::new()
        .post(notes_url(&server, run_id))
        .json(&json!({ "note": "no auth" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "unauthenticated POST must be 401");

    let resp = reqwest::Client::new()
        .get(notes_url(&server, run_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "unauthenticated GET must be 401");
}

#[tokio::test]
async fn post_run_note_requires_background_use_permission() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_notes_owner_perm").await;
    let run_id = insert_subagent_run(&server, &owner.user_id).await;

    // Authenticated but WITHOUT `background::use` → 403 (not 404 — the perm gate
    // fires before ownership resolution).
    let noperm = create_user_with_permissions(&server, "bg_notes_noperm", &[]).await;
    let resp = reqwest::Client::new()
        .post(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .json(&json!({ "note": "denied" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "missing background::use must be 403");
}

#[tokio::test]
async fn enqueue_then_list_roundtrip_owner() {
    let server = TestServer::start().await;
    let user = bg_user(&server, "bg_notes_roundtrip").await;
    let run_id = insert_subagent_run(&server, &user.user_id).await;
    let client = reqwest::Client::new();

    // Enqueue → 201 + the persisted RunNote.
    let post = client
        .post(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "note": "  focus on the 2024 revision  " }))
        .send()
        .await
        .unwrap();
    assert_eq!(post.status(), 201, "owner enqueue must be 201");
    let body: serde_json::Value = post.json().await.unwrap();
    assert_eq!(body["run_id"], run_id.to_string());
    assert_eq!(body["note"], "focus on the 2024 revision", "note is trimmed");
    assert!(body["consumed_at"].is_null(), "a fresh note is pending");

    // List → 200 with the pending note.
    let list = client
        .get(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let items: serde_json::Value = list.json().await.unwrap();
    let arr = items.as_array().expect("list is an array");
    assert_eq!(arr.len(), 1, "the enqueued note is listed");
    assert_eq!(arr[0]["note"], "focus on the 2024 revision");

    // Empty note → 400.
    let bad = client
        .post(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "note": "   " }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400, "empty note must be 400");
}

#[tokio::test]
async fn cross_user_run_is_404() {
    let server = TestServer::start().await;
    let owner = bg_user(&server, "bg_notes_owner").await;
    let other = bg_user(&server, "bg_notes_other").await;
    let run_id = insert_subagent_run(&server, &owner.user_id).await;
    let client = reqwest::Client::new();

    // User B (has background::use, but not this run) → 404 on POST and GET.
    let post = client
        .post(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "note": "not my run" }))
        .send()
        .await
        .unwrap();
    assert_eq!(post.status(), 404, "cross-user POST must be 404 (never leak)");

    let get = client
        .get(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404, "cross-user GET must be 404 (never leak)");

    // And the owner still sees no leaked note from B's rejected attempts.
    let list = client
        .get(notes_url(&server, run_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let items: serde_json::Value = list.json().await.unwrap();
    assert_eq!(items.as_array().unwrap().len(), 0, "B's attempts enqueued nothing");
}
