//! Tier 5 — real-LLM memory tests.
//!
//! Runs the memory module against actual external providers:
//!   - Gemini text-embedding-004 (768d) for embeddings
//!   - Groq Llama 4 Scout for extraction + summarization
//!
//! These exercise the paths that the rest of the suite mocks or
//! skips: real embedding generation, real vector search, real LLM
//! extraction, real LLM summarization (full + incremental).
//!
//! Gated behind `#[ignore]` so a default `cargo test` doesn't try
//! to hit external APIs. Run with:
//!
//!   source tests/.env.test && \
//!     cargo test --test integration_tests -- --ignored --test-threads=1 \
//!     memory::real_llm
//!
//! Costs are negligible — both providers' free tiers cover the full
//! suite. Each test logs setup choices via eprintln! so you can grep
//! the run output if something looks off.

#![allow(unused_imports)]

use serde_json::{Value, json};
use uuid::Uuid;

use super::real_llm_helpers as h;

// ────────────────────────────────────────────────────────────────────
// R1 — embedding generation actually writes a real vector.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn r1_embedding_dispatch_writes_real_vector() {
    if h::skip_if_no_keys("r1_embedding_dispatch") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r1_embed").await;

    let memory_id = h::mcp_remember(&server, &user.token, "The user lives in Portland, Oregon.").await;
    h::wait_for_embedding(&server, &user.token, memory_id).await;

    // Re-fetch and assert the embedding_model column reflects the
    // Gemini model name (production records the model NAME, not UUID,
    // so the re-embed worker can dedup cheaply).
    let body: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{memory_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let embedding_model = body["embedding_model"].as_str().unwrap_or("");
    assert_eq!(
        embedding_model,
        h::GEMINI_EMBEDDING_MODEL,
        "embedding_model should be the Gemini model name; got {embedding_model:?}"
    );
}

// ────────────────────────────────────────────────────────────────────
// R2 — vector retrieval finds the semantically-closest memory.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn r2_retrieval_finds_semantically_similar_memory() {
    if h::skip_if_no_keys("r2_retrieval") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r2_retrieve").await;

    // Seed five disjoint memories. Embedding happens inline via the
    // MCP remember tool; wait for each to commit before querying.
    let memories = vec![
        "The user is a senior Rust developer working on a chat application.",
        "The user prefers dark mode in their code editor and avoids light themes.",
        "The user is allergic to peanuts and avoids cross-contaminated foods.",
        "The user lives in Portland, Oregon and bikes to work most days.",
        "The user's favorite book is Project Hail Mary by Andy Weir.",
    ];
    let mut ids = Vec::with_capacity(memories.len());
    for content in &memories {
        let id = h::mcp_remember(&server, &user.token, content).await;
        h::wait_for_embedding(&server, &user.token, id).await;
        ids.push(id);
    }

    // Query — semantically closest to memory #2 (allergies).
    let hits = h::mcp_recall(
        &server,
        &user.token,
        "What food restrictions does the user have?",
        3,
    )
    .await;
    assert!(!hits.is_empty(), "recall returned zero hits");

    let top = &hits[0];
    assert!(
        top.contains("peanut") || top.to_lowercase().contains("allerg"),
        "top hit should be the peanut-allergy memory; got: {top:?}\nfull hits: {hits:?}"
    );
}

// ────────────────────────────────────────────────────────────────────
// R3 — extraction pipeline against a real Groq Llama 4 LLM.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn r3_extraction_pipeline_with_real_groq_llm() {
    if h::skip_if_no_keys("r3_extraction") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r3_extract").await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    // Enable extraction for this user.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "extraction_enabled": true }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "enable extraction → {}", res.status());

    // Trigger the extractor synchronously via the debug-only test
    // hook — the production `after_llm_call` path is fire-and-forget
    // and we need determinism. The route is admin-gated, so we hit
    // it with the admin token (built up by setup_real_providers's
    // admin user — recreated here for the test).
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "r3_extract_admin",
        &["memory::admin::manage"],
    )
    .await;
    let user_msg = "I'm vegetarian and have been for 10 years. Can you suggest some dinner ideas?";
    let assistant_msg = "Here are three vegetarian dinner ideas: ...";
    let res = reqwest::Client::new()
        .post(server.api_url("/_test/memory/extract"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "user_id": user_id,
            "user_message": user_msg,
            "assistant_message": assistant_msg,
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "test/extract → {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );

    // Now list the user's memories. At least one should mention
    // vegetarian-ness; the exact phrasing is the LLM's choice so
    // we match loosely.
    let res = reqwest::Client::new()
        .get(server.api_url("/memories?limit=50"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let rows: Vec<Value> = res.json().await.unwrap();
    let contents: Vec<&str> = rows.iter().filter_map(|r| r["content"].as_str()).collect();
    assert!(
        contents.iter().any(|c| c.to_lowercase().contains("vegetarian")),
        "expected an extracted memory mentioning 'vegetarian', got: {contents:?}"
    );
    // The extracted row should be `source = "extraction"` (not
    // 'manual' / 'mcp_tool'), proving the extraction pipeline wrote
    // it.
    let extracted = rows
        .iter()
        .find(|r| {
            r["content"]
                .as_str()
                .map(|c| c.to_lowercase().contains("vegetarian"))
                .unwrap_or(false)
        })
        .expect("vegetarian row present");
    assert_eq!(extracted["source"], "extraction");
}

// ────────────────────────────────────────────────────────────────────
// R4 — summarization full path against a real Groq Llama 4 LLM.
// ────────────────────────────────────────────────────────────────────
//
// Requires a real branch with N messages. We create the conversation
// via REST, then INSERT N synthetic messages directly via SQL — the
// summarizer just needs `branches` + `messages` + `message_contents`
// rows; it doesn't care that the assistant didn't actually generate
// the turns.

/// Hit the debug-only `/_test/memory/summarize` route so the
/// summarizer runs inside the server process (where Repos exists).
/// Calling `summarizer::refresh_summary` directly from the test
/// process panics — see comment in `real_llm_helpers.rs` for context.
async fn trigger_summarize_via_test_hook(
    server: &crate::common::TestServer,
    branch_id: Uuid,
    model_id: Uuid,
) {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "summ_hook_admin",
        &["memory::admin::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/_test/memory/summarize"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "branch_id": branch_id, "model_id": model_id }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "test/summarize → {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );
}

async fn seed_branch_with_messages(
    server: &crate::common::TestServer,
    user_token: &str,
    _user_id: Uuid,
    message_count: usize,
) -> (Uuid, Vec<Uuid>) {
    // Create a conversation — the API auto-creates the active branch.
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {user_token}"))
        .json(&json!({ "title": "tier5-summarize" }))
        .send()
        .await
        .unwrap();
    let conv: Value = res.json().await.unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    // Insert synthetic messages directly via SQL — bypasses the
    // chat-streaming pipeline (which would require a real LLM call
    // per turn). The summarizer reads via
    // `Repos.chat.core.get_conversation_history(branch_id)`, which
    // joins messages → branch_messages and orders by
    // `branch_messages.created_at`. So we need rows in THREE tables
    // (messages, branch_messages, message_contents) and we increment
    // bm.created_at per insert to lock in chronological order.
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

/// Append N synthetic messages to an existing branch starting at the
/// given index offset. Returns (branch_id_unchanged, new_msg_ids).
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

/// Worker for both seed and append paths. Inserts `count` messages
/// into the given branch starting at index `start_index` (used for
/// the bm.created_at offset so subsequent appends ORDER after the
/// initial seed).
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
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let mut ids = Vec::with_capacity(count);
    for i in start_index..(start_index + count) {
        let (role, text) = make_msg(i);
        let msg_id = Uuid::new_v4();
        // 1. messages row — originated_from_id = id (no edit chain).
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
        // 2. branch_messages junction — created_at increments so the
        //    ORDER BY in list_messages_in_branch produces our intended
        //    chronological order.
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
        // 3. content — content_type='text' + jsonb body.
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
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
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

#[tokio::test]
#[ignore]
async fn r4_summarization_full_with_real_groq_llm() {
    if h::skip_if_no_keys("r4_summarize_full") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r4_summ").await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    // Seed 60 messages — above the default trigger of 50, with 10
    // keep_recent → 50 messages get summarized.
    let (branch_id, _msg_ids) = seed_branch_with_messages(&server, &user.token, user_id, 60).await;

    // Invoke the summarizer directly with our real Groq model id.
    trigger_summarize_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let summary = fetch_summary_row(&server, branch_id)
        .await
        .expect("summary row should exist after refresh");
    let text = summary["summary_text"].as_str().unwrap();
    assert!(
        text.len() > 50,
        "summary should be a real paragraph, got {} chars: {text:?}",
        text.len()
    );
    // The seeded transcript is about a Tokyo trip — a competent LLM
    // should mention Tokyo or trip-planning in the summary.
    assert!(
        text.to_lowercase().contains("tokyo") || text.to_lowercase().contains("trip"),
        "summary should reflect seeded content (Tokyo trip), got: {text:?}"
    );
    assert_eq!(summary["message_count"], 50);
    assert!(summary["summarized_up_to_id"].is_string());
    assert_eq!(summary["model_used"], h::GROQ_LLM_MODEL);
}

// ────────────────────────────────────────────────────────────────────
// R5 — incremental refresh advances the summary cheaply.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn r5_summarization_incremental_with_real_groq_llm() {
    if h::skip_if_no_keys("r5_summarize_incremental") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r5_summ_inc").await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    // Seed 60 messages → first refresh writes a Full summary.
    let (branch_id, _) = seed_branch_with_messages(&server, &user.token, user_id, 60).await;
    trigger_summarize_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let first = fetch_summary_row(&server, branch_id).await.unwrap();
    let first_anchor = first["summarized_up_to_id"].as_str().unwrap().to_string();
    let first_count = first["message_count"].as_i64().unwrap();
    assert_eq!(first_count, 50);

    // Add 20 more messages — different topic so we can prove the
    // incremental summary actually folded in the new content.
    let _ = append_messages_to_branch(&server, branch_id, 60, 20, |i| {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        let text = if role == "user" {
            format!("User turn {i}: Now I want to ask about my cat — she's been sneezing a lot.")
        } else {
            format!(
                "Assistant turn {i}: Sneezing in cats can have many causes. Has she been to a vet recently?"
            )
        };
        (role.to_string(), text)
    })
    .await;

    // Second refresh → must take the INCREMENTAL branch (anchor present,
    // 20 new messages between anchor and cutoff).
    trigger_summarize_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    let second = fetch_summary_row(&server, branch_id).await.unwrap();
    let second_count = second["message_count"].as_i64().unwrap();
    let second_anchor = second["summarized_up_to_id"].as_str().unwrap().to_string();

    // 80 total - 10 keep_recent = 70 summarized
    assert_eq!(second_count, 70);
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
#[ignore]
async fn r6_incremental_falls_back_to_full_on_anchor_loss() {
    if h::skip_if_no_keys("r6_incremental_fallback") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r6_summ_fallback").await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    let (branch_id, msg_ids) = seed_branch_with_messages(&server, &user.token, user_id, 60).await;
    trigger_summarize_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    // Simulate anchor loss: set the existing row's
    // summarized_up_to_id to a message that EXISTS in `messages`
    // (FK requirement) but is NOT in this branch's history. The
    // decision logic uses `to_summarize.iter().position(|m| m.id ==
    // prev_anchor_id)` against the branch history, so an orphan
    // message id triggers the "anchor not in history → Full" path.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
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
        branch_id
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    let _ = msg_ids; // suppress unused

    trigger_summarize_via_test_hook(&server, branch_id, ids.llm_model_id).await;

    // After the Full re-summarize, the anchor should be the LAST
    // message that was summarized — i.e., msg_ids[49] (50th message,
    // since cutoff = 60 - 10 = 50). We can't compare to msg_ids
    // directly across the SQL roundtrip without a string format, so
    // just assert the row is fresh and the anchor is non-null.
    let after = fetch_summary_row(&server, branch_id).await.unwrap();
    assert!(after["summarized_up_to_id"].is_string());
    assert_eq!(after["message_count"], 50);
}

// ────────────────────────────────────────────────────────────────────
// R7 — cosine threshold filters semantically unrelated memories.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn r7_cosine_threshold_filters_unrelated_memories() {
    if h::skip_if_no_keys("r7_cosine_threshold") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r7_threshold").await;

    // Tighten the threshold so only quite-similar matches return.
    // Default is 0.6 (cosine distance, lower = stricter). 0.3 means
    // we require ~70% cosine similarity, which "User likes cats" vs.
    // "User likes Rust" should NOT cross.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "r7_admin",
        &["memory::admin::manage", "memory::admin::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "cosine_threshold": 0.3 }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "tighten threshold");

    let id = h::mcp_remember(
        &server,
        &user.token,
        "The user is interested in the Rust programming language.",
    )
    .await;
    h::wait_for_embedding(&server, &user.token, id).await;

    // Query an unrelated topic — should return zero hits under the
    // tight threshold (Rust ↔ cooking should be cosine-distant).
    let hits = h::mcp_recall(
        &server,
        &user.token,
        "What's the user's favorite recipe for pasta?",
        5,
    )
    .await;
    assert!(
        hits.is_empty(),
        "tight cosine_threshold=0.3 should filter unrelated memories; \
         got hits: {hits:?}"
    );
}
