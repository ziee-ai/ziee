// ============================================================================
// Embedding worker failure-skip path (embedding_worker.rs: embed() Err => log +
// skip, no UPDATE, no panic, continue). When the admin points memory at an
// embedding model whose provider is unreachable, the re-embed worker spawned by
// the admin-settings PUT must FAIL GRACEFULLY: the existing memory row keeps a
// NULL embedding (never falsely marked embedded), CRUD keeps working, and the
// server stays healthy. Deterministic + key-free — the embed call fails because
// the provider base_url points at a closed loopback port (connection refused).
// ============================================================================

use serde_json::{json, Value};

async fn perm_user(server: &crate::common::TestServer) -> crate::common::test_helpers::TestUser {
    crate::common::test_helpers::create_user_with_permissions(
        server,
        "embed_fail",
        &[
            "memory::read",
            "memory::write",
            "memory::admin::read",
            "memory::admin::manage",
            "llm_providers::read",
            "llm_providers::create",
            "llm_models::read",
            "llm_models::create",
        ],
    )
    .await
}

#[tokio::test]
async fn test_embedding_failure_skips_row_without_breaking_crud() {
    let server = crate::common::TestServer::start().await;
    let user = perm_user(&server).await;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", user.token);

    // A provider whose base_url is a closed loopback port → every embed call
    // gets connection-refused (no network, no key needed).
    let provider: Value = client
        .post(server.api_url("/llm-providers"))
        .header("Authorization", &auth)
        .json(&json!({
            "name": "Broken Embedder",
            "provider_type": "openai",
            "api_key": "x",
            "base_url": "http://127.0.0.1:9/v1",
            "enabled": true
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let model: Value = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", &auth)
        .json(&json!({
            "provider_id": provider_id,
            "name": "broken-embed",
            "display_name": "Broken Embed",
            "description": "unreachable embedding model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "text_embedding": true }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let embedding_model_id = model["id"].as_str().unwrap().to_string();

    // Seed a memory BEFORE enabling embedding → it has a NULL embedding.
    let mem: Value = client
        .post(server.api_url("/memories"))
        .header("Authorization", &auth)
        .json(&json!({ "content": "The user lives in Helsinki." }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mem_id = mem["id"].as_str().unwrap().to_string();

    // Enable memory + point it at the broken embedder → spawns the re-embed
    // worker, which will try (and fail) to embed the seeded row.
    let put = client
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", &auth)
        .json(&json!({ "enabled": true, "embedding_model_id": embedding_model_id }))
        .send()
        .await
        .unwrap();
    assert!(put.status().is_success(), "admin-settings PUT: {}", put.status());

    // Give the background worker time to attempt + fail the embed.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // The row is intact and NOT falsely marked embedded (the failure skipped it).
    let got: Value = client
        .get(server.api_url(&format!("/memories/{mem_id}")))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got["content"], "The user lives in Helsinki.", "row intact: {got}");
    assert!(
        got["embedding_model"].is_null(),
        "embed failed ⇒ embedding_model must stay NULL (row skipped, not corrupted): {got}"
    );

    // CRUD still works after the embed failure (worker did not crash the server).
    let mem2 = client
        .post(server.api_url("/memories"))
        .header("Authorization", &auth)
        .json(&json!({ "content": "The user prefers tea." }))
        .send()
        .await
        .unwrap();
    assert_eq!(mem2.status(), reqwest::StatusCode::CREATED, "CRUD survives embed failure");

    // The deployment is still healthy/serving admin reads.
    let settings = client
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", &auth)
        .send()
        .await
        .unwrap();
    assert!(settings.status().is_success(), "server healthy after embed failure");
}
