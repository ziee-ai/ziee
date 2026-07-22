//! Shared setup for cross-subsystem tests that drive the memory
//! `remember`/`recall` MCP tools alongside another subsystem.
//!
//! Memory now defaults OFF deployment-wide (`memory_admin_settings.enabled`
//! seeds FALSE — the privacy-safe default), so `recall` is rejected with
//! "memory is disabled by the administrator" until an admin enables it AND
//! configures an embedding model. The cross-subsystem recall queries are
//! natural language, so the FTS-only arm (AND-semantics) won't match — a real
//! embedding model is required for the semantic arm.
//!
//! This enables memory against the local embedding bridge: it configures the
//! built-in "Google Gemini" provider (redirected at the test bridge via the
//! `GEMINI_BASE_URL` seam), creates a `gemini-embedding-001` embedding model,
//! turns memory on wired to that model, waits for the dimension probe +
//! `ALTER COLUMN` to land (the model is 3072-dim; the column starts 768-dim, and
//! the inline `remember` embed write would silently fail against the old width),
//! and enables retrieval for the given user.
//!
//! The user MUST already hold `memory::admin::{read,manage}`,
//! `llm_providers::{read,edit}`, `llm_models::create` (plus `memory::read` /
//! `memory::write` to remember/recall its own facts).

use serde_json::{Value, json};

use crate::common::TestServer;

/// `true` (skip the test) when `GEMINI_API_KEY` is unset — the local embedding
/// bridge is keyed off it (`source tests/.env.test`).
pub fn skip_if_no_embedding_key(test: &str) -> bool {
    if std::env::var("GEMINI_API_KEY").is_err() {
        eprintln!("test {test} skipped: GEMINI_API_KEY unset (source tests/.env.test)");
        return true;
    }
    false
}

/// Configure the built-in Gemini provider + a `gemini-embedding-001` embedding
/// model against the test bridge, enable memory deployment-wide wired to that
/// model, wait for the embedding column to be resized to the model's dimension,
/// and enable retrieval for `token`'s user. Returns the embedding model id.
pub async fn enable_semantic_memory(server: &TestServer, token: &str) -> String {
    let client = reqwest::Client::new();

    // 1. Configure the built-in "Google Gemini" provider. The base_url seam
    //    (`GEMINI_BASE_URL` via `test_provider_base_url`) redirects it at the
    //    local embedding bridge — WITHOUT it the provider hits the real Google
    //    endpoint and the probe embed fails (INVALID_EMBEDDING_MODEL).
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY (source tests/.env.test)");
    let providers: Value = client
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("GET providers")
        .json()
        .await
        .expect("providers json");
    let provider = providers["providers"]
        .as_array()
        .expect("providers array")
        .iter()
        .find(|p| p["name"].as_str() == Some("Google Gemini"))
        .expect("built-in 'Google Gemini' provider")
        .clone();
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let mut provider_payload = json!({ "api_key": api_key, "enabled": true });
    if let Some(base_url) = crate::chat::helpers::test_provider_base_url("GEMINI_API_KEY") {
        provider_payload["base_url"] = json!(base_url);
    }
    let res = client
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&provider_payload)
        .send()
        .await
        .expect("POST provider");
    assert!(
        res.status().is_success(),
        "configure Gemini provider: {}",
        res.text().await.unwrap_or_default()
    );

    // 2. Create the embedding model.
    let res = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": "gemini-embedding-001",
            "display_name": "Gemini Embedding 001",
            "description": "cross-subsystem memory test embedding model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "text_embedding": true },
        }))
        .send()
        .await
        .expect("POST llm-models");
    assert_eq!(
        res.status(),
        reqwest::StatusCode::CREATED,
        "create embedding model: {}",
        res.text().await.unwrap_or_default()
    );
    let model: Value = res.json().await.expect("model json");
    let embedding_model_id = model["id"].as_str().unwrap().to_string();

    // Read the column's current dimension so we can detect when the async
    // probe + ALTER (below) has resized it.
    let start_dim = admin_dimension(&client, server, token).await;

    // 3. Enable memory deployment-wide + wire the embedding model. This kicks off
    //    a fire-and-forget dimension probe + `reembed_all` that ALTERs the
    //    embedding column to the model's dimension.
    let res = client
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "enabled": true, "embedding_model_id": embedding_model_id }))
        .send()
        .await
        .expect("PUT memory/admin-settings");
    assert!(
        res.status().is_success(),
        "enable memory admin-settings: {} {:?}",
        res.status(),
        res.text().await
    );

    // 4. Wait for the ALTER COLUMN to land. The `remember` tool embeds inline and
    //    writes the vector straight into `user_memories.embedding`; against the
    //    old (narrower) column width that write silently fails and the row keeps a
    //    NULL embedding → the semantic recall arm never finds it. Poll until the
    //    recorded dimension changes from the pre-enable value.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let dim = admin_dimension(&client, server, token).await;
        if dim != start_dim {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "memory embedding dimension never changed from {start_dim} (probe/ALTER did not land)"
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    // 5. Enable retrieval for this user.
    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "retrieval_enabled": true }))
        .send()
        .await
        .expect("PUT /memory/settings");
    assert!(res.status().is_success(), "enable user retrieval: {}", res.status());

    embedding_model_id
}

/// Current `embedding_dimensions` from `GET /memory/admin-settings`.
async fn admin_dimension(client: &reqwest::Client, server: &TestServer, token: &str) -> i64 {
    let body: Value = client
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("GET memory/admin-settings")
        .json()
        .await
        .expect("admin-settings json");
    body["embedding_dimensions"].as_i64().expect("embedding_dimensions")
}
