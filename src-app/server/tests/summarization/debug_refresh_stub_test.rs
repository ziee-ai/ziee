//! Deterministic (stub-LLM) coverage for two summarization gaps:
//!   - all-573a722f1e44: the debug `/_test/summarization/refresh` endpoint had
//!     no integration test — nothing drove a synchronous refresh through it.
//!   - all-f00b3df66fbc: nothing asserted the admin's CUSTOM prompt override is
//!     actually USED in the summarizer's LLM call (the existing test only
//!     round-trips the setting through storage).
//!
//! Both run against a capturing stub provider, so the full
//! refresh_summary → LLM-call path executes for real; only token generation is
//! canned. The custom prompt's unique marker must appear in the recorded
//! request, proving the override reached the model.

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::stub_chat::{StubChat, register_stub_model};
use crate::common::test_helpers::{TestUser, create_user_with_permissions};
use crate::common::TestServer;

async fn open_pool(server: &TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Create a conversation and seed `count` text messages on its active branch via
/// SQL (bypasses the streaming pipeline). Returns the active-branch id.
async fn seed_branch(server: &TestServer, user_token: &str, count: usize) -> Uuid {
    let conv: Value = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {user_token}"))
        .json(&json!({ "title": "summ-stub" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    let pool = open_pool(server).await;
    for i in 0..count {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        let text = format!(
            "Turn {i} ({role}): We discussed the quarterly migration plan, the staging \
             rollout schedule, and the rollback criteria for the database cutover in detail."
        );
        let msg_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
               VALUES ($1, $2, $1, 0, NOW() + ($3::int * INTERVAL '1 second'))"#,
            msg_id,
            role,
            i as i32,
        )
        .execute(&pool)
        .await
        .expect("insert message");
        sqlx::query!(
            r#"INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
               VALUES ($1, $2, false, NOW() + ($3::int * INTERVAL '1 second'))"#,
            branch_id,
            msg_id,
            i as i32,
        )
        .execute(&pool)
        .await
        .expect("insert branch_message");
        sqlx::query!(
            r#"INSERT INTO message_contents (message_id, content_type, content, sequence_order)
               VALUES ($1, 'text', $2, 0)"#,
            msg_id,
            json!({ "type": "text", "text": text }),
        )
        .execute(&pool)
        .await
        .expect("insert message content");
    }
    pool.close().await;
    branch_id
}

async fn admin(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "summarization::settings::manage",
            "conversations::create",
            "conversations::read",
            "llm_providers::create",
            "llm_providers::read",
            "llm_models::create",
            "llm_models::read",
            // register_stub_model also creates + assigns a group; without these
            // POST /groups returned no id and the helper panicked on unwrap.
            "llm_providers::assign_groups",
            "groups::create",
            "groups::assign_users",
        ],
    )
    .await
}

async fn put_settings(server: &TestServer, token: &str, body: Value) {
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "put settings: {}", res.text().await.unwrap_or_default());
}

async fn trigger_refresh(server: &TestServer, token: &str, branch_id: Uuid, model_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/_test/summarization/refresh"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "branch_id": branch_id, "model_id": model_id }))
        .send()
        .await
        .unwrap()
}

async fn summary_row_exists(server: &TestServer, branch_id: Uuid) -> bool {
    let pool = open_pool(server).await;
    let row = sqlx::query!(
        "SELECT 1 as one FROM conversation_summaries WHERE branch_id = $1",
        branch_id
    )
    .fetch_optional(&pool)
    .await
    .expect("query summary");
    pool.close().await;
    row.is_some()
}

// all-573a722f1e44 — the debug refresh endpoint drives a synchronous summary.
#[tokio::test]
async fn debug_refresh_endpoint_produces_a_summary() {
    let server = TestServer::start().await;
    let user = admin(&server, "summ_dbg_refresh").await;
    let stub = StubChat::start().await;
    let model_id = register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, false, None).await;
    let model_id = Uuid::parse_str(&model_id).unwrap();

    // Low thresholds so the seeded transcript triggers summarization.
    put_settings(&server, &user.token, json!({ "summarize_after_tokens": 500, "summarizer_keep_recent_tokens": 100 })).await;
    let branch_id = seed_branch(&server, &user.token, 40).await;

    let res = trigger_refresh(&server, &user.token, branch_id, model_id).await;
    assert!(res.status().is_success(), "refresh endpoint: {}", res.text().await.unwrap_or_default());

    assert!(
        summary_row_exists(&server, branch_id).await,
        "the debug refresh must produce a conversation_summaries row"
    );
    // The summarizer actually called the (stub) model.
    assert!(
        !stub.requests().is_empty(),
        "the refresh must drive a real LLM call through the summarizer"
    );
}

// all-f00b3df66fbc — the admin's custom full-summary prompt override is actually
// USED in the summarizer's LLM call (not just stored).
#[tokio::test]
async fn custom_full_summary_prompt_is_used_in_the_llm_call() {
    let server = TestServer::start().await;
    let user = admin(&server, "summ_custom_prompt").await;
    let stub = StubChat::start().await;
    let model_id = register_stub_model(&server, &user.token, &user.user_id, &stub.base_url, false, None).await;
    let model_id = Uuid::parse_str(&model_id).unwrap();

    const MARKER: &str = "ZIEE_SUMPROMPT_MARKER_5521";
    put_settings(
        &server,
        &user.token,
        json!({
            "summarize_after_tokens": 500,
            "summarizer_keep_recent_tokens": 100,
            // Must contain the {transcript} placeholder (validated server-side).
            "full_summary_prompt": format!("{MARKER} Summarize the following conversation:\n{{transcript}}"),
        }),
    )
    .await;
    let branch_id = seed_branch(&server, &user.token, 40).await;

    let res = trigger_refresh(&server, &user.token, branch_id, model_id).await;
    assert!(res.status().is_success(), "refresh endpoint: {}", res.text().await.unwrap_or_default());

    let reqs = stub.requests();
    let last = reqs.last().expect("the summarizer made an LLM request");
    assert!(
        last.all_text.contains(MARKER),
        "the custom full_summary_prompt override must be used in the LLM call (marker missing)"
    );
}
