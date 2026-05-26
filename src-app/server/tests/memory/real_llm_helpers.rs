//! Shared test infrastructure for Tier-5 real-LLM memory tests.
//!
//! Sets up real providers:
//!   - Gemini (`text-embedding-004`, 768d) for embeddings — picked
//!     because (a) the dim matches `memory_admin_settings.embedding_dimensions=768`
//!     default so no column-rebuild on first use, (b) free tier
//!     covers the test workload, (c) `gemini` provider type is
//!     already registered in `ai-providers::registry`.
//!   - Groq (`meta-llama/llama-4-scout-17b-16e-instruct`) for chat —
//!     fast, free tier, OpenAI-compatible API surface that the
//!     existing OpenAI provider transparently handles. No embedding
//!     model here; Groq doesn't offer encoders.
//!
//! Keys come from `tests/.env.test` — `GEMINI_API_KEY` and
//! `GROQ_API_KEY`. Tests skip with an eprintln! if either is missing.

#![allow(dead_code)]

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::TestUser;

// As of 2026-05, Gemini's only embedContent-supporting models are
// `gemini-embedding-001` (3072d), `gemini-embedding-2`, and
// `-2-preview`. `text-embedding-004` was retired. We use `-001` and
// rely on the admin handler's dimension-probe worker to ALTER the
// vector column from the migration's default vector(768) → vector(3072)
// the first time this model is set.
pub const GEMINI_EMBEDDING_MODEL: &str = "gemini-embedding-001";
pub const GROQ_LLM_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

/// Returns the IDs of the models registered by `setup_real_providers`.
pub struct RealProviderIds {
    pub embedding_model_id: Uuid,
    pub llm_model_id: Uuid,
}

/// Skip-gate. Returns `true` (skip) and prints a message if either
/// API key is missing. Use in test setup:
///
///   if real_llm_helpers::skip_if_no_keys("test_name").await { return; }
pub fn skip_if_no_keys(test_name: &str) -> bool {
    let gemini = std::env::var("GEMINI_API_KEY").is_ok();
    let groq = std::env::var("GROQ_API_KEY").is_ok();
    if !gemini || !groq {
        eprintln!(
            "test {test_name} skipped: missing key(s) — gemini={gemini} groq={groq}. \
             Set GEMINI_API_KEY + GROQ_API_KEY (typically via `source tests/.env.test`)."
        );
        return true;
    }
    false
}

/// Provision real providers + models + admin settings end-to-end so
/// the rest of the test can drive memory through real LLM/embedding
/// paths. Returns the model IDs the test will reference via
/// `memory_admin_settings`.
pub async fn setup_real_providers(server: &TestServer) -> RealProviderIds {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "memory_real_admin",
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
    let token = admin.token.clone();

    let gemini = configure_builtin_provider(server, &token, "Google Gemini", "GEMINI_API_KEY").await;
    let groq = configure_builtin_provider(server, &token, "Groq", "GROQ_API_KEY").await;

    let embedding_model = create_model(
        server,
        &token,
        &gemini["id"].as_str().unwrap(),
        GEMINI_EMBEDDING_MODEL,
        "Gemini text-embedding-004",
        json!({ "text_embedding": true }),
    )
    .await;
    let llm_model = create_model(
        server,
        &token,
        &groq["id"].as_str().unwrap(),
        GROQ_LLM_MODEL,
        "Groq Llama 4 Scout",
        json!({ "chat": true, "tools": false }),
    )
    .await;

    let embedding_model_id = Uuid::parse_str(embedding_model["id"].as_str().unwrap()).unwrap();
    let llm_model_id = Uuid::parse_str(llm_model["id"].as_str().unwrap()).unwrap();

    // Configure memory: enable + wire both models.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "enabled": true,
            "embedding_model_id": embedding_model_id,
            "default_extraction_model_id": llm_model_id,
        }))
        .send()
        .await
        .expect("PUT memory/admin-settings");
    assert!(
        res.status().is_success(),
        "memory admin-settings PUT failed: {} {:?}",
        res.status(),
        res.text().await
    );

    RealProviderIds {
        embedding_model_id,
        llm_model_id,
    }
}

/// Find a built-in provider by display name, set its api_key + enable.
/// Mirrors `chat::helpers::configure_provider_with_api_key`.
async fn configure_builtin_provider(
    server: &TestServer,
    token: &str,
    display_name: &str,
    env_var: &str,
) -> Value {
    let api_key = std::env::var(env_var).expect("API key env var");

    let res = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("GET providers");
    let body: Value = res.json().await.expect("providers json");
    let providers = body["providers"].as_array().expect("providers array");
    let provider = providers
        .iter()
        .find(|p| p["name"].as_str() == Some(display_name))
        .unwrap_or_else(|| panic!("built-in provider '{display_name}' not found"))
        .clone();
    let provider_id = provider["id"].as_str().unwrap();

    // The provider-update endpoint takes POST (not PUT) — see
    // llm_provider/routes.rs.
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "api_key": api_key,
            "enabled": true,
        }))
        .send()
        .await
        .expect("POST provider");
    let status = res.status();
    let body = res.text().await.expect("provider body");
    assert!(
        status.is_success(),
        "configure provider {display_name} failed: {status}: {body}"
    );
    serde_json::from_str(&body).expect("updated provider json")
}

async fn create_model(
    server: &TestServer,
    token: &str,
    provider_id: &str,
    model_name: &str,
    display_name: &str,
    capabilities: Value,
) -> Value {
    let payload = json!({
        "provider_id": provider_id,
        "name": model_name,
        "display_name": display_name,
        "description": format!("Memory Tier-5 test model: {model_name}"),
        "enabled": true,
        "engine_type": "none",
        "file_format": "gguf",
        "capabilities": capabilities,
    });
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .expect("POST llm-models");
    let status = res.status();
    let body = res.text().await.expect("model body");
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create model {model_name} → {status}: {body}"
    );
    serde_json::from_str(&body).expect("model json")
}

// ────────────────────────────────────────────────────────────────────
// MCP `remember` — embedded inline by the production path. Use this
// instead of POST /memories when you need the row to have an embedding
// for retrieval tests. Returns the inserted memory id.
// ────────────────────────────────────────────────────────────────────

pub async fn mcp_remember(server: &TestServer, token: &str, content: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "remember",
                "arguments": { "content": content },
            },
        }))
        .send()
        .await
        .expect("MCP remember POST");
    let body: Value = res.json().await.expect("MCP body");
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("MCP remember returned no text: {body}"));
    // The remember tool returns "Remembered. id=<uuid>" or similar —
    // we parse the UUID out of the structured content.
    let structured = &body["result"]["structuredContent"];
    if let Some(id) = structured.get("memory_id").and_then(|v| v.as_str()) {
        return Uuid::parse_str(id).expect("uuid");
    }
    // Fall back: scan the text for a uuid pattern.
    for tok in text.split_whitespace() {
        if let Ok(id) = Uuid::parse_str(tok.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-')) {
            return id;
        }
    }
    panic!("could not extract memory_id from MCP remember response: {body}");
}

/// MCP `recall` — query for similar memories. Returns list of content strings.
pub async fn mcp_recall(server: &TestServer, token: &str, query: &str, top_k: i64) -> Vec<String> {
    let res = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "recall",
                "arguments": { "query": query, "top_k": top_k },
            },
        }))
        .send()
        .await
        .expect("MCP recall POST");
    let body: Value = res.json().await.expect("MCP body");
    // Structured content: { "memories": [{"content": "...", ...}, ...] }
    body["result"]["structuredContent"]["memories"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| m["content"].as_str().map(String::from))
        .collect()
}

// ────────────────────────────────────────────────────────────────────
// Test users for memory ops.
// ────────────────────────────────────────────────────────────────────

pub async fn memory_user(server: &TestServer, name: &str) -> TestUser {
    crate::common::test_helpers::create_user_with_permissions(
        server,
        name,
        &["memory::read", "memory::write"],
    )
    .await
}

/// Poll GET /memories/{id} until `embedding_model` is non-null.
/// The MCP remember path embeds inline, but the response is sent
/// before the embedding write-back commits — so a brief poll covers
/// the gap. Returns `Ok(())` on success or panics on timeout.
pub async fn wait_for_embedding(server: &TestServer, token: &str, memory_id: Uuid) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let client = reqwest::Client::new();
    let url = server.api_url(&format!("/memories/{memory_id}"));
    loop {
        let res = client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("GET memory");
        if res.status().is_success() {
            let body: Value = res.json().await.expect("memory body");
            if !body["embedding_model"].is_null() {
                return;
            }
        }
        if std::time::Instant::now() > deadline {
            panic!("embedding never landed for memory {memory_id}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}
