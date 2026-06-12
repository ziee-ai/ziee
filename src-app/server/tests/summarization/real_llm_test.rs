//! Tier 5 — real-LLM summarization tests.
//!
//! Runs the summarizer against an actual external provider (Groq
//! Llama 4 Scout). These exercise the paths that the rest of the
//! suite mocks or skips: real LLM summarization (full + incremental).
//!
//! Run with the rest of the suite:
//!
//!   source tests/.env.test && \
//!     cargo test --test integration_tests -- --test-threads=1 \
//!         summarization::
//!
//! Self-skips via `skip_if_no_keys` when `GROQ_API_KEY` is absent —
//! `tests/.env.test` ships a working key so the tests run by default.
//!
//! Tests were R4/R5/R6 in the memory module's `real_llm_test.rs` prior
//! to migration 91; moved here as part of the summarization extraction.

#![allow(clippy::too_many_lines)]

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::real_llm_helpers as h;

// ────────────────────────────────────────────────────────────────────
// Helpers private to the Tier-5 tests.
// ────────────────────────────────────────────────────────────────────

/// Hit the debug-only `/_test/summarization/refresh` route so the
/// summarizer runs inside the server process (where `Repos` exists).
/// Calling `summarizer::refresh_summary` directly from the test
/// process would require the global Repos init, which the test
/// harness doesn't perform.
async fn trigger_refresh_via_test_hook(
    server: &crate::common::TestServer,
    branch_id: Uuid,
    model_id: Uuid,
) {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "summ_hook_admin",
        &["summarization::settings::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/_test/summarization/refresh"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "branch_id": branch_id, "model_id": model_id }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "test/summarization/refresh → {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );
}

/// Lower the summarizer token thresholds so a handful of short
/// seeded messages trigger summarization. The validation floor is
/// 500 / 100 (see migration 91 CHECK constraints).
async fn set_low_summary_thresholds(server: &crate::common::TestServer) {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "summ_threshold_admin",
        &["summarization::settings::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "summarize_after_tokens": 500,
            "summarizer_keep_recent_tokens": 100,
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "set summary thresholds → {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );
}

async fn open_pool(server: &crate::common::TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Create a conversation + insert `count` synthetic messages directly
/// via SQL. Bypasses the chat-streaming pipeline (which would require
/// a real LLM call per turn). Returns the active-branch id +
/// the seeded message ids.
async fn seed_branch_with_messages(
    server: &crate::common::TestServer,
    user_token: &str,
    message_count: usize,
) -> (Uuid, Vec<Uuid>) {
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {user_token}"))
        .json(&json!({ "title": "tier5-summarize" }))
        .send()
        .await
        .unwrap();
    let conv: Value = res.json().await.unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    seed_messages_at(server, branch_id, 0, message_count, |i| {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        let text = if role == "user" {
            format!("User turn {i}: I'm planning a trip to Tokyo next month.")
        } else {
            format!(
                "Assistant turn {i}: Great! Tokyo in spring is wonderful. Any specific neighborhoods you're interested in?"
            )
        };
        (role.to_string(), text)
    })
    .await
}

async fn append_messages_to_branch<F>(
    server: &crate::common::TestServer,
    branch_id: Uuid,
    start_index: usize,
    count: usize,
    make_msg: F,
) -> Vec<Uuid>
where
    F: Fn(usize) -> (String, String),
{
    let (_, ids) = seed_messages_at(server, branch_id, start_index, count, make_msg).await;
    ids
}

async fn seed_messages_at<F>(
    server: &crate::common::TestServer,
    branch_id: Uuid,
    start_index: usize,
    count: usize,
    make_msg: F,
) -> (Uuid, Vec<Uuid>)
where
    F: Fn(usize) -> (String, String),
{
    let pool = open_pool(server).await;
    let mut ids = Vec::with_capacity(count);
    for i in start_index..(start_index + count) {
        let (role, text) = make_msg(i);
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
        ids.push(msg_id);
    }
    pool.close().await;
    (branch_id, ids)
}

async fn fetch_summary_row(
    server: &crate::common::TestServer,
    branch_id: Uuid,
) -> Option<Value> {
    let pool = open_pool(server).await;
    let row = sqlx::query!(
        r#"SELECT summary_text, summarized_up_to_id, message_count, model_used
           FROM conversation_summaries WHERE branch_id = $1"#,
        branch_id
    )
    .fetch_optional(&pool)
    .await
    .expect("query summary");
    pool.close().await;
    row.map(|r| {
        json!({
            "summary_text": r.summary_text,
            "summarized_up_to_id": r.summarized_up_to_id.map(|u| u.to_string()),
            "message_count": r.message_count,
            "model_used": r.model_used,
        })
    })
}

// ────────────────────────────────────────────────────────────────────
// R4 — summarization full path against a real Groq Llama 4 LLM.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r4_summarization_full_with_real_groq_llm() {
    if h::skip_if_no_keys("r4_summarization_full") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::summarization_user(&server, "r4_summ").await;

    set_low_summary_thresholds(&server).await;
    let (branch_id, _msg_ids) =
        seed_branch_with_messages(&server, &user.token, 60).await;

    trigger_refresh_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let summary = fetch_summary_row(&server, branch_id)
        .await
        .expect("summary row should exist after refresh");
    let text = summary["summary_text"].as_str().unwrap();
    assert!(
        text.len() > 50,
        "summary should be a real paragraph, got {} chars: {text:?}",
        text.len()
    );
    assert!(
        text.to_lowercase().contains("tokyo") || text.to_lowercase().contains("trip"),
        "summary should reflect seeded content (Tokyo trip), got: {text:?}"
    );
    assert!(
        summary["message_count"].as_i64().unwrap_or(0) > 0,
        "summarization should have folded an older prefix; got {summary:?}"
    );
    assert!(summary["summarized_up_to_id"].is_string());
    assert_eq!(summary["model_used"], h::GROQ_LLM_MODEL);
}

// ────────────────────────────────────────────────────────────────────
// R5 — incremental refresh advances the summary cheaply.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r5_summarization_incremental_with_real_groq_llm() {
    if h::skip_if_no_keys("r5_summarization_incremental") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::summarization_user(&server, "r5_summ_inc").await;

    set_low_summary_thresholds(&server).await;
    let (branch_id, _) = seed_branch_with_messages(&server, &user.token, 60).await;
    trigger_refresh_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let first = fetch_summary_row(&server, branch_id).await.unwrap();
    let first_anchor = first["summarized_up_to_id"].as_str().unwrap().to_string();
    let first_count = first["message_count"].as_i64().unwrap();
    assert!(first_count > 0, "first refresh should summarize an older prefix");

    // Append 20 new turns on a fresh topic — proof the incremental
    // summary folds in the new content (not just rewrites the old).
    let _ = append_messages_to_branch(&server, branch_id, 60, 20, |i| {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        let text = if role == "user" {
            format!(
                "User turn {i}: Now I want to ask about my cat — she's been sneezing a lot."
            )
        } else {
            format!(
                "Assistant turn {i}: Sneezing in cats can have many causes. Has she been to a vet recently?"
            )
        };
        (role.to_string(), text)
    })
    .await;

    trigger_refresh_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let second = fetch_summary_row(&server, branch_id).await.unwrap();
    let second_count = second["message_count"].as_i64().unwrap();
    let second_anchor = second["summarized_up_to_id"].as_str().unwrap().to_string();

    assert!(
        second_count > first_count,
        "incremental refresh should summarize more (first={first_count}, second={second_count})"
    );
    assert_ne!(
        first_anchor, second_anchor,
        "anchor should advance after incremental refresh"
    );
    let text = second["summary_text"].as_str().unwrap();
    assert!(
        text.to_lowercase().contains("cat") || text.to_lowercase().contains("sneez"),
        "incremental summary should reflect the new cat-sneezing turns, got: {text:?}"
    );
}

// ────────────────────────────────────────────────────────────────────
// R6 — incremental falls back to FULL when the anchor is lost.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r6_incremental_falls_back_to_full_on_anchor_loss() {
    if h::skip_if_no_keys("r6_incremental_fallback") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::summarization_user(&server, "r6_summ_fallback").await;

    set_low_summary_thresholds(&server).await;
    let (branch_id, _msg_ids) =
        seed_branch_with_messages(&server, &user.token, 60).await;
    trigger_refresh_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    // Simulate anchor loss: set the existing row's
    // summarized_up_to_id to a message that EXISTS in `messages`
    // (FK requirement) but is NOT in this branch's history. The
    // decision logic uses `to_summarize.iter().position(|m| m.id ==
    // prev_anchor_id)` against the branch history, so an orphan
    // message id triggers the "anchor not in history → Full" path.
    let pool = open_pool(&server).await;
    let orphan_msg_id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO messages (id, role, originated_from_id, edit_count)
           VALUES ($1, 'user', $1, 0)"#,
        orphan_msg_id,
    )
    .execute(&pool)
    .await
    .expect("insert orphan message");
    sqlx::query!(
        r#"UPDATE conversation_summaries SET summarized_up_to_id = $1 WHERE branch_id = $2"#,
        orphan_msg_id,
        branch_id,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    trigger_refresh_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let after = fetch_summary_row(&server, branch_id).await.unwrap();
    assert!(after["summarized_up_to_id"].is_string());
    assert!(
        after["message_count"].as_i64().unwrap_or(0) > 0,
        "the Full fallback re-summarize should fold an older prefix; got {after:?}"
    );
}
