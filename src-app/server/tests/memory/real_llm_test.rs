//! Tier 5 — real-LLM memory tests.
//!
//! Runs the memory module against actual external providers:
//!   - Gemini text-embedding-004 (768d) for embeddings
//!   - Groq Llama 4 Scout for extraction
//!
//! These exercise the paths that the rest of the suite mocks or
//! skips: real embedding generation, real vector search, real LLM
//! extraction.
//!
//! Summarization Tier-5 tests (R4/R5/R6) moved with the engine to
//! `tests/summarization/real_llm_test.rs` in migration 91.
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
    // `/memories` returns the paginated `MemoryListResponse { items, total, … }`.
    let body: Value = res.json().await.unwrap();
    let rows = body["items"].as_array().cloned().unwrap_or_default();
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
// R7 — cosine threshold filters semantically unrelated memories.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
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

// ────────────────────────────────────────────────────────────────────
// R8-R10 — embedding-model swap / rebuild paths.
//
// These cover what R1-R7 didn't:
//   R8  — same-dim model swap (no ALTER, but every row gets re-embedded)
//   R9  — dim-down swap (3072 → 1536; tests ALTER + index recreate)
//   R10 — explicit re-embed endpoint resumes after stale embedding_model
//
// Use OpenAI for ada-002 (1536d) and text-embedding-3-small (1536d) —
// the two non-Gemini real embedding models we have credentials for.
// Free-tier-friendly for tests.
// ────────────────────────────────────────────────────────────────────

const OPENAI_EMBEDDING_MODEL_A: &str = "text-embedding-ada-002";
const OPENAI_EMBEDDING_MODEL_B: &str = "text-embedding-3-small";

/// Register the OpenAI provider + two embedding models (same dim:
/// 1536). Returns (model_a_id, model_b_id). Reuses an admin user with
/// the right perms; caller provides the user separately for inserts.
async fn register_two_openai_embedders(
    server: &crate::common::TestServer,
) -> (Uuid, Uuid) {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "openai_embed_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
            "memory::admin::read",
            "memory::admin::manage",
        ],
    )
    .await;
    let provider = h::configure_builtin_provider(server, &admin.token, "OpenAI", "OPENAI_API_KEY").await;
    let pid = provider["id"].as_str().unwrap();
    let model_a = h::create_model(
        server,
        &admin.token,
        pid,
        OPENAI_EMBEDDING_MODEL_A,
        "OpenAI ada-002 (embed test A)",
        json!({ "text_embedding": true }),
    )
    .await;
    let model_b = h::create_model(
        server,
        &admin.token,
        pid,
        OPENAI_EMBEDDING_MODEL_B,
        "OpenAI 3-small (embed test B)",
        json!({ "text_embedding": true }),
    )
    .await;
    (
        Uuid::parse_str(model_a["id"].as_str().unwrap()).unwrap(),
        Uuid::parse_str(model_b["id"].as_str().unwrap()).unwrap(),
    )
}

/// Set the admin embedding model + enable. Returns the admin token
/// (caller reuses for subsequent swap PUTs).
async fn set_embedding_model(
    server: &crate::common::TestServer,
    model_id: Uuid,
) -> String {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "swap_admin",
        &["memory::admin::manage", "memory::admin::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "enabled": true,
            "embedding_model_id": model_id,
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "PUT memory/admin-settings → {}",
        res.status()
    );
    admin.token
}

/// Poll the public rebuild-status endpoint until either it reports
/// in_progress=false AND pending_count=0, or the deadline expires.
async fn wait_for_rebuild_done(
    server: &crate::common::TestServer,
    admin_token: &str,
    timeout_secs: u64,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let client = reqwest::Client::new();
    let url = server.api_url("/memory/admin-settings/rebuild-status");
    loop {
        let res = client
            .get(&url)
            .header("Authorization", format!("Bearer {admin_token}"))
            .send()
            .await
            .unwrap();
        let body: Value = res.json().await.unwrap();
        let in_progress = body["in_progress"].as_bool().unwrap_or(false);
        let pending = body["pending_count"].as_i64().unwrap_or(0);
        if !in_progress && pending == 0 {
            return;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "rebuild didn't complete in {timeout_secs}s: in_progress={in_progress} pending={pending}"
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn r8_same_dim_model_swap_rebuilds_all_rows() {
    if h::skip_if_no_openai("r8_same_dim_swap") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let (model_a, model_b) = register_two_openai_embedders(&server).await;
    let admin_token = set_embedding_model(&server, model_a).await;
    let user = h::memory_user(&server, "r8_user").await;

    // Seed 3 memories under model A. Their embedding_model column
    // should reflect ada-002 once the embed lands.
    let mut ids = Vec::new();
    for content in [
        "User likes the Rust programming language.",
        "User is allergic to peanuts.",
        "User lives in Portland, Oregon.",
    ] {
        let id = h::mcp_remember(&server, &user.token, content).await;
        h::wait_for_embedding(&server, &user.token, id).await;
        ids.push(id);
    }
    for id in &ids {
        let body: Value = reqwest::Client::new()
            .get(server.api_url(&format!("/memories/{id}")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["embedding_model"], OPENAI_EMBEDDING_MODEL_A);
    }

    // Snapshot the dim — should be 1536 for ada-002.
    let before: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before["embedding_dimensions"], 1536);

    // Swap to model B (same dim 1536). Worker should re-embed all
    // 3 rows without ALTER.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "embedding_model_id": model_b }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    // Wait for the worker to settle. Same-dim re-embed: 3 rows ×
    // ~1s each = ~3s.
    wait_for_rebuild_done(&server, &admin_token, 30).await;

    // Re-check each row — should now show model B's name.
    for id in &ids {
        let body: Value = reqwest::Client::new()
            .get(server.api_url(&format!("/memories/{id}")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            body["embedding_model"], OPENAI_EMBEDDING_MODEL_B,
            "row {id} should be re-embedded with model B"
        );
    }

    // Dim should stay at 1536 — no ALTER fired.
    let after: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["embedding_dimensions"], 1536);
}

#[tokio::test]
async fn r9_dim_down_change_alters_column_and_reembeds() {
    if h::skip_if_no_keys("r9_dim_down") || h::skip_if_no_openai("r9_dim_down") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _gemini_ids = h::setup_real_providers(&server).await;
    // Gemini (3072d) is now wired. Register OpenAI 3-small (1536d) as
    // the dim-down target.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "r9_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
            "memory::admin::read",
            "memory::admin::manage",
        ],
    )
    .await;
    let openai = h::configure_builtin_provider(&server, &admin.token, "OpenAI", "OPENAI_API_KEY").await;
    let three_small = h::create_model(
        &server,
        &admin.token,
        openai["id"].as_str().unwrap(),
        OPENAI_EMBEDDING_MODEL_B,
        "OpenAI 3-small (r9)",
        json!({ "text_embedding": true }),
    )
    .await;
    let three_small_id = Uuid::parse_str(three_small["id"].as_str().unwrap()).unwrap();

    // Insert a memory under Gemini. Wait for embedding (3072d).
    let user = h::memory_user(&server, "r9_user").await;
    let mem_id = h::mcp_remember(
        &server,
        &user.token,
        "The user's favorite cuisine is Vietnamese.",
    )
    .await;
    h::wait_for_embedding(&server, &user.token, mem_id).await;

    // Confirm we're at 3072.
    let before: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before["embedding_dimensions"], 3072);

    // Swap to 3-small (1536d). Worker NULLs + ALTERs halfvec(3072) →
    // halfvec(1536), drops/recreates hnsw index, re-embeds.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "embedding_model_id": three_small_id }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    wait_for_rebuild_done(&server, &admin.token, 60).await;

    // Dim should now be 1536.
    let after: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["embedding_dimensions"], 1536);

    // The seeded row should be re-embedded with 3-small.
    let body: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{mem_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["embedding_model"], OPENAI_EMBEDDING_MODEL_B);

    // And retrieval should still work via the recreated hnsw index.
    let hits = h::mcp_recall(&server, &user.token, "what food does the user prefer?", 3).await;
    assert!(!hits.is_empty(), "retrieval should work post-rebuild");
}

#[tokio::test]
async fn r10_explicit_reembed_endpoint_resumes_stale_rows() {
    if h::skip_if_no_openai("r10_explicit_reembed") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let (model_a, _) = register_two_openai_embedders(&server).await;
    let admin_token = set_embedding_model(&server, model_a).await;
    let user = h::memory_user(&server, "r10_user").await;

    // Seed 3 memories under ada-002.
    let mut ids = Vec::new();
    for content in [
        "User enjoys jazz music.",
        "User has a cat named Mochi.",
        "User is learning to play piano.",
    ] {
        let id = h::mcp_remember(&server, &user.token, content).await;
        h::wait_for_embedding(&server, &user.token, id).await;
        ids.push(id);
    }

    // Pretend 2 of the 3 rows are stale: rewrite their embedding_model
    // to a value that doesn't match the current admin model. The
    // worker's re-embed filter is `embedding_model IS DISTINCT FROM
    // current.name OR embedding IS NULL` — so these two rows should
    // be re-embedded on the next reembed call.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    sqlx::query!(
        "UPDATE user_memories SET embedding_model = 'pretend-stale-model' WHERE id IN ($1, $2)",
        ids[0],
        ids[1]
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Confirm rebuild-status reports 2 pending.
    let status_url = server.api_url("/memory/admin-settings/rebuild-status");
    let status: Value = reqwest::Client::new()
        .get(&status_url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["pending_count"], 2);
    assert_eq!(status["in_progress"], false);

    // Trigger explicit re-embed. The handler probes the current
    // model + spawns the worker WITHOUT requiring a model_id change.
    let res = reqwest::Client::new()
        .post(server.api_url("/memory/admin-settings/reembed"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success() || res.status() == 202,
        "reembed trigger → {}",
        res.status()
    );

    wait_for_rebuild_done(&server, &admin_token, 30).await;

    // The two stale rows should now report ada-002 again; the third
    // was already current and unchanged.
    for id in &ids {
        let body: Value = reqwest::Client::new()
            .get(server.api_url(&format!("/memories/{id}")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["embedding_model"], OPENAI_EMBEDDING_MODEL_A);
    }

    // Final status: nothing pending.
    let status: Value = reqwest::Client::new()
        .get(&status_url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["pending_count"], 0);
    assert_eq!(status["in_progress"], false);
}

// ────────────────────────────────────────────────────────────────────
// R-forget — the MCP `forget` tool soft-deletes a memory so a later
// recall no longer surfaces it (real embeddings + real vector search).
// The existing `test_mcp_forget_requires_memory_id` only covers the
// missing-arg validation; this exercises the full remember→recall→
// forget→recall lifecycle end to end.
// ────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn r_forget_removes_memory_from_subsequent_recall() {
    if h::skip_if_no_keys("r_forget") {
        return;
    }
    let server = crate::common::TestServer::start().await;
    let _ids = h::setup_real_providers(&server).await;
    let user = h::memory_user(&server, "r_forget").await;

    // A distinctive, isolated fact so recall is unambiguous.
    let fact = "The user's emergency contact passphrase is ORCHID-DELTA-77.";
    let id = h::mcp_remember(&server, &user.token, fact).await;
    h::wait_for_embedding(&server, &user.token, id).await;

    // Before forgetting: recall surfaces it.
    let before = h::mcp_recall(&server, &user.token, "emergency contact passphrase", 5).await;
    assert!(
        before.iter().any(|m| m.contains("ORCHID-DELTA-77")),
        "recall must find the memory before forget; got {before:?}"
    );

    // Forget it via the MCP `forget` tool (soft-delete).
    let forget: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "forget", "arguments": { "memory_id": id.to_string() } },
        }))
        .send()
        .await
        .expect("forget POST")
        .json()
        .await
        .expect("forget body");
    assert!(forget["error"].is_null(), "forget must succeed: {forget}");

    // After forgetting: recall no longer surfaces it.
    let after = h::mcp_recall(&server, &user.token, "emergency contact passphrase", 5).await;
    assert!(
        !after.iter().any(|m| m.contains("ORCHID-DELTA-77")),
        "forgotten memory must NOT reappear in recall; got {after:?}"
    );

    // And the row is gone from the owner's REST list (soft-deleted).
    let got = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 404, "a forgotten memory must 404 on direct GET");
}
