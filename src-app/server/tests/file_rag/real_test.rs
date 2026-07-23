use serde_json::json;
use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;
use super::attach_conversation_to_project;
use super::attach_file_to_project;
use super::create_project;
use super::db_pool;
use super::project_conversation_with_files;
use super::semantic_search;
use super::set_rag_settings;
use super::upload_text;
use super::wait_for_chunks;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::test_helpers::TestUser;
use crate::common::TestServer;

const GEMINI_EMBEDDING_MODEL: &str = "gemini-embedding-001";

const ANTHROPIC_HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

fn skip_if_no_gemini(test: &str) -> bool {
    if std::env::var("GEMINI_API_KEY").is_err() {
        eprintln!("test {test} skipped: GEMINI_API_KEY unset (source tests/.env.test)");
        return true;
    }
    false
}

/// Enable a built-in provider with its API key. Minimal copy of
/// memory's `real_llm_helpers::configure_builtin_provider` (that module is
/// private to the memory test tree).
async fn configure_provider(server: &TestServer, token: &str, display_name: &str, env_var: &str) -> String {
    let api_key = std::env::var(env_var).expect("api key");
    let res = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("GET providers");
    let body: Value = res.json().await.unwrap();
    let provider = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"].as_str() == Some(display_name))
        .unwrap_or_else(|| panic!("provider {display_name} not found"))
        .clone();
    let provider_id = provider["id"].as_str().unwrap().to_string();
    // Redirect at a local embeddings bridge (per-provider `<PROVIDER>_BASE_URL`
    // or the global `ZIEE_TEST_LLM_BASE_URL`) — same seam memory's Tier-5 helper
    // uses. Without it the provider points at the real Google endpoint and the
    // probe embed fails (INVALID_EMBEDDING_MODEL).
    let mut provider_payload = json!({ "api_key": api_key, "enabled": true });
    if let Some(base_url) = crate::chat::helpers::test_provider_base_url(env_var) {
        provider_payload["base_url"] = json!(base_url);
    }
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&provider_payload)
        .send()
        .await
        .expect("POST provider");
    assert!(res.status().is_success(), "configure provider: {}", res.text().await.unwrap_or_default());
    provider_id
}

async fn create_embedding_model(server: &TestServer, token: &str, provider_id: &str, name: &str) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": name,
            "display_name": name,
            "description": "file_rag Tier-3 embedding model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "text_embedding": true },
        }))
        .send()
        .await
        .expect("POST llm-models");
    assert_eq!(res.status(), reqwest::StatusCode::CREATED, "create model: {}", res.text().await.unwrap_or_default());
    let body: Value = res.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

/// Poll until every chunk of a file has a non-NULL embedding (real Gemini calls
/// + the column ALTER take a few seconds).
async fn wait_for_embeddings(pool: &PgPool, file_id: &str) {
    let fid = Uuid::parse_str(file_id).unwrap();
    for _ in 0..120 {
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM file_chunks WHERE file_id = $1 AND embedding IS NULL",
        )
        .bind(fid)
        .fetch_one(pool)
        .await
        .expect("count pending");
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks WHERE file_id = $1")
            .bind(fid)
            .fetch_one(pool)
            .await
            .expect("count total");
        if total > 0 && pending == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("embeddings never landed for file {file_id}");
}

async fn admin_dimension(server: &TestServer, user: &TestUser) -> i64 {
    let res = reqwest::Client::new()
        .get(server.api_url("/file-rag/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    body["embedding_dimensions"].as_i64().unwrap()
}

/// Real embeddings rank a semantically-related doc above a lexically-similar
/// distractor — and the 768→3072 dimension rebuild + re-embed of existing
/// chunks works end-to-end.
#[tokio::test]
async fn real_embedder_semantic_ranking_and_rebuild() {
    if skip_if_no_gemini("real_embedder_semantic_ranking_and_rebuild") {
        return;
    }
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let admin = create_user_with_permissions(&server, "file_rag_real", &["*"]).await;

    // Index two distinct docs FIRST (no embedder yet → chunks with NULL
    // embeddings, FTS-ready). Doing it before configuring the model means the
    // rebuild re-embeds existing chunks (and there's no eager-embed/ALTER race).
    let (conv, ids) = project_conversation_with_files(
        &server,
        &admin,
        "rag-real",
        &[
            (
                "biology.txt",
                "Mitochondria generate adenosine triphosphate through oxidative phosphorylation, \
                 supplying the chemical fuel that drives most cellular activity in eukaryotes.",
            ),
            (
                "history.txt",
                "The storming of the Bastille in 1789 marked the beginning of the French Revolution \
                 and the eventual collapse of the Bourbon monarchy.",
            ),
        ],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;
    wait_for_chunks(&pool, &ids[1], 1).await;

    // Configure a real Gemini embedder (3072-dim). The PUT probes the dimension
    // and spawns the rebuild: ALTER halfvec(768)→halfvec(3072) + re-embed.
    let provider_id = configure_provider(&server, &admin.token, "Google Gemini", "GEMINI_API_KEY").await;
    let model_id = create_embedding_model(&server, &admin.token, &provider_id, GEMINI_EMBEDDING_MODEL).await;
    set_rag_settings(&server, &admin, json!({ "embedding_model_id": model_id })).await;

    // The dimension was probe-derived to 3072 and the column rebuilt.
    for _ in 0..60 {
        if admin_dimension(&server, &admin).await == 3072 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(admin_dimension(&server, &admin).await, 3072, "embedding_dimensions probe-derived to 3072");

    // The rebuild re-embeds the existing chunks of both docs.
    wait_for_embeddings(&pool, &ids[0]).await;
    wait_for_embeddings(&pool, &ids[1]).await;

    // A semantically-related, lexically-different query must surface the
    // biology doc above the history distractor — only real vectors do this.
    let body = semantic_search(&server, &admin, conv_uuid, "how do cells produce energy").await;
    assert!(body["error"].is_null(), "semantic_search should succeed; body={body}");
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["mode"].as_str().unwrap(), "hybrid", "embedder configured → hybrid mode");
    let results = sc["results"].as_array().unwrap();
    assert!(!results.is_empty(), "results must be non-empty");
    assert_eq!(
        results[0]["file_id"].as_str().unwrap(),
        ids[0].as_str(),
        "the biology doc must out-rank the history distractor on a semantic query; results={results:?}"
    );
}

/// Pure **vector-only** mode (`mode:"vector"`): embedder configured AND
/// `fts_enabled=false`, so the lexical arm is off entirely. A query with NO
/// lexical overlap with the target ("cellular energy production" vs the doc's
/// "oxidative phosphorylation / adenosine triphosphate") would return nothing
/// from FTS — so a correct top hit proves the vector arm carried the search on
/// its own. Completes the mode matrix (fts/hybrid are covered above; this is
/// the (has_vector=true, fts_enabled=false)→Arm::Vector path) with real vectors.
#[tokio::test]
async fn real_embedder_vector_only_mode() {
    if skip_if_no_gemini("real_embedder_vector_only_mode") {
        return;
    }
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let admin = create_user_with_permissions(&server, "file_rag_vec_only", &["*"]).await;

    let (conv, ids) = project_conversation_with_files(
        &server,
        &admin,
        "rag-vec-only",
        &[
            (
                "biology.txt",
                "Mitochondria generate adenosine triphosphate through oxidative phosphorylation, \
                 supplying the chemical fuel that drives most cellular activity in eukaryotes.",
            ),
            (
                "history.txt",
                "The storming of the Bastille in 1789 marked the beginning of the French Revolution \
                 and the eventual collapse of the Bourbon monarchy.",
            ),
        ],
    )
    .await;
    let conv_uuid = Uuid::parse_str(&conv).unwrap();
    wait_for_chunks(&pool, &ids[0], 1).await;
    wait_for_chunks(&pool, &ids[1], 1).await;

    let provider_id = configure_provider(&server, &admin.token, "Google Gemini", "GEMINI_API_KEY").await;
    let model_id = create_embedding_model(&server, &admin.token, &provider_id, GEMINI_EMBEDDING_MODEL).await;
    set_rag_settings(&server, &admin, json!({ "embedding_model_id": model_id })).await;
    for _ in 0..60 {
        if admin_dimension(&server, &admin).await == 3072 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(admin_dimension(&server, &admin).await, 3072, "embedding_dimensions probe-derived to 3072");
    wait_for_embeddings(&pool, &ids[0]).await;
    wait_for_embeddings(&pool, &ids[1]).await;

    // Turn the lexical arm OFF — search must now run vector-only.
    set_rag_settings(&server, &admin, json!({ "fts_enabled": false })).await;

    let body = semantic_search(&server, &admin, conv_uuid, "cellular energy production").await;
    assert!(body["error"].is_null(), "semantic_search should succeed; body={body}");
    let sc = &body["result"]["structuredContent"];
    assert_eq!(
        sc["mode"].as_str().unwrap(),
        "vector",
        "embedder on + fts_enabled=false → vector-only mode"
    );
    let results = sc["results"].as_array().unwrap();
    assert!(
        !results.is_empty(),
        "vector arm must return hits on a lexically-disjoint query (FTS is off); body={body}"
    );
    assert_eq!(
        results[0]["file_id"].as_str().unwrap(),
        ids[0].as_str(),
        "vector-only search must still surface the biology doc; results={results:?}"
    );
}

/// Resolve a real Anthropic Haiku chat model (configures the built-in provider
/// with the key). Returns None when ANTHROPIC_API_KEY is unset (test skips).
async fn anthropic_haiku_model(server: &TestServer, user_id: &str) -> Option<Value> {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return None;
    }
    let cfg = crate::chat::helpers::TestModelConfig {
        provider_type: "anthropic",
        model_name: ANTHROPIC_HAIKU_MODEL,
        display_name: "Claude Haiku 4.5",
    };
    let m = crate::chat::helpers::create_test_model_with_config(server, &cfg, Some(user_id)).await;
    if m.is_null() {
        None
    } else {
        Some(m)
    }
}

/// The agentic end-to-end: a REAL chat model (Claude Haiku) is given only a
/// *manifest* of a project doc — its body is NOT inlined (project knowledge is
/// manifest-only for tool-capable models) — and must DECIDE to call the
/// `semantic_search` MCP tool to answer a question whose answer lives only in
/// the doc body, then ground its reply in the retrieved passage.
///
/// Asserts BOTH that `semantic_search` specifically fired (`mcpToolStart`) AND
/// that the buried fact reached the answer. This is sound either way: if the
/// doc were inlined, the model would answer without the tool and the
/// `mcpToolStart` assertion fails loudly — no silent false-positive. Runs in
/// FTS mode (only ANTHROPIC_API_KEY needed); real-vector ranking is covered by
/// the tests above. Mirrors `tests/chat/sandbox_real_llm_test.rs`'s
/// real-LLM-called-a-specific-tool pattern.
#[tokio::test]
async fn real_llm_calls_semantic_search_and_grounds_answer() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!(
            "test real_llm_calls_semantic_search_and_grounds_answer skipped: \
             ANTHROPIC_API_KEY unset (source tests/.env.test)"
        );
        return;
    }
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = create_user_with_permissions(&server, "file_rag_agentic", &["*"]).await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        return;
    };
    let model_id = model["id"].as_str().unwrap().to_string();

    // A distinctive fact buried in the body. Neither the filename nor the
    // manifest reveals it — the ONLY way to surface ZARQON-7741 is to search
    // the doc's chunks. Filler keeps the fact off any "first line" heuristic.
    let filler = "General safety guidance and routine operating procedures follow standard \
                  company policy and are reviewed annually by the operations team. "
        .repeat(40);
    let body = format!(
        "Helsinki Operations Handbook — internal reference.\n\n{filler}\n\n\
         Section 12.4 — Evacuation: the designated emergency rendezvous codeword for the \
         Helsinki facility is ZARQON-7741. Personnel must present this exact codeword at the \
         assembly point before re-entry is authorized.\n\n{filler}\n"
    );

    let project_id = create_project(&server, &user, "rag-agentic").await;
    let file_id = upload_text(&server, &user, "helsinki-ops-handbook.txt", &body).await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    wait_for_chunks(&pool, &file_id, 1).await; // FTS-ready

    // Conversation pinned to the real Haiku model, attached to the project so
    // its knowledge files are in scope (as a manifest, not inlined).
    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("create conv");
    assert_eq!(conv_resp.status(), reqwest::StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    attach_conversation_to_project(&server, &user, &project_id, &conv["id"].as_str().unwrap().to_string()).await;

    let turn = crate::chat::helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        Uuid::parse_str(&model_id).unwrap(),
        "Use the semantic_search tool to look up the Helsinki facility's emergency rendezvous \
         codeword in the project documents, then tell me the exact codeword.",
    )
    .await;

    // 1) The real model chose OUR tool (not grep_files/read_file, not a bare guess).
    let tools_fired: Vec<&str> = turn
        .frames
        .iter()
        .filter(|f| f.event_type == "mcpToolStart")
        .filter_map(|f| f.data["tool_name"].as_str())
        .collect();
    assert!(
        tools_fired.contains(&"semantic_search"),
        "real LLM must call semantic_search; tools fired={tools_fired:?}; answer={:?}",
        turn.text
    );
    // 2) ...and it grounded the answer in the retrieved passage.
    assert!(
        turn.text.contains("ZARQON-7741"),
        "answer must contain the codeword retrieved via semantic_search; answer={:?}",
        turn.text
    );
}

/// Real-LLM `read_file` through the agentic loop (the stub test
/// `agentic_chat::manifest_injected_and_read_file_round_trips` covers the stub
/// path; the existing real-LLM test exercises `semantic_search`, not
/// `read_file`). A distinctive codeword is buried in a manifest-attached doc;
/// the model is asked to READ the file and report it, and we assert the real
/// model actually fired `read_file` and grounded its answer in the content.
#[tokio::test]
async fn real_llm_calls_read_file_through_agentic_loop() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!(
            "test real_llm_calls_read_file_through_agentic_loop skipped: \
             ANTHROPIC_API_KEY unset (source tests/.env.test)"
        );
        return;
    }
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = create_user_with_permissions(&server, "file_rag_readfile", &["*"]).await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        return;
    };
    let model_id = model["id"].as_str().unwrap().to_string();

    let filler = "Routine operating procedures follow standard company policy. ".repeat(30);
    let body = format!(
        "Reykjavik Operations Handbook — internal reference.\n\n{filler}\n\n\
         Section 7.1 — Access: the master door-lock override code for the Reykjavik vault is \
         GLACIER-5582. Do not share outside operations.\n\n{filler}\n"
    );

    let project_id = create_project(&server, &user, "readfile-agentic").await;
    let file_id = upload_text(&server, &user, "reykjavik-ops-handbook.txt", &body).await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    wait_for_chunks(&pool, &file_id, 1).await;

    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("create conv");
    assert_eq!(conv_resp.status(), reqwest::StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    attach_conversation_to_project(
        &server,
        &user,
        &project_id,
        &conv["id"].as_str().unwrap().to_string(),
    )
    .await;

    let turn = crate::chat::helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        Uuid::parse_str(&model_id).unwrap(),
        "Open the Reykjavik operations handbook with the read_file tool and tell me the exact \
         master door-lock override code written in it.",
    )
    .await;

    let tools_fired: Vec<&str> = turn
        .frames
        .iter()
        .filter(|f| f.event_type == "mcpToolStart")
        .filter_map(|f| f.data["tool_name"].as_str())
        .collect();
    assert!(
        tools_fired.contains(&"read_file"),
        "real LLM must call read_file through the agentic loop; tools fired={tools_fired:?}; answer={:?}",
        turn.text
    );
    assert!(
        turn.text.contains("GLACIER-5582"),
        "answer must contain the code read via read_file; answer={:?}",
        turn.text
    );
}

/// Agentic read_file via a REAL model: the same manifest-only setup, but the
/// answer lives in a NAMED section the model must OPEN with the `read_file`
/// tool (not semantic_search). Asserts `read_file` specifically fired AND the
/// buried fact reached the answer — the existing agentic read_file test uses a
/// STUB model, so this is the real-LLM path through the manifest→read_file loop.
#[tokio::test]
async fn real_llm_calls_read_file_through_agentic_loop_v2() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!(
            "test real_llm_calls_read_file_through_agentic_loop skipped: ANTHROPIC_API_KEY unset"
        );
        return;
    }
    let server = TestServer::start().await;
    let pool = db_pool(&server).await;
    let user = create_user_with_permissions(&server, "file_rag_readfile", &["*"]).await;

    let Some(model) = anthropic_haiku_model(&server, &user.user_id).await else {
        return;
    };
    let model_id = model["id"].as_str().unwrap().to_string();

    let filler = "Routine operating procedures follow standard company policy and are \
                  reviewed annually by the operations team. "
        .repeat(30);
    let body = format!(
        "Reykjavik Logistics Handbook — internal reference.\n\n{filler}\n\n\
         Section 7.2 — Cold-Chain Override: the manual override passphrase for the \
         Reykjavik freezer bank is FROST-KELDA-0934. Operators must enter this exact \
         passphrase to bypass the automatic lock.\n\n{filler}\n"
    );

    let project_id = create_project(&server, &user, "readfile-agentic").await;
    let file_id = upload_text(&server, &user, "reykjavik-logistics-handbook.txt", &body).await;
    attach_file_to_project(&server, &user, &project_id, &file_id).await;
    wait_for_chunks(&pool, &file_id, 1).await;

    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("create conv");
    assert_eq!(conv_resp.status(), reqwest::StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    attach_conversation_to_project(
        &server,
        &user,
        &project_id,
        &conv["id"].as_str().unwrap().to_string(),
    )
    .await;

    let turn = crate::chat::helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        Uuid::parse_str(&model_id).unwrap(),
        "Use the read_file tool to open the file 'reykjavik-logistics-handbook.txt' from this \
         project and tell me the exact Cold-Chain Override passphrase in section 7.2.",
    )
    .await;

    let tools_fired: Vec<&str> = turn
        .frames
        .iter()
        .filter(|f| f.event_type == "mcpToolStart")
        .filter_map(|f| f.data["tool_name"].as_str())
        .collect();
    assert!(
        tools_fired.contains(&"read_file"),
        "real LLM must call read_file; tools fired={tools_fired:?}; answer={:?}",
        turn.text
    );
    assert!(
        turn.text.contains("FROST-KELDA-0934"),
        "answer must contain the passphrase read via read_file; answer={:?}",
        turn.text
    );
}

