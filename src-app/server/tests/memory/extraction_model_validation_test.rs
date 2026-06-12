// ============================================================================
// Extraction-model capability validation.
//
// Memory's silent extraction/summarization pipeline calls chat_stream()
// on the configured `default_extraction_model_id`. An *embedding* model
// can't generate text — it's served in llama.cpp `--embeddings` mode, so
// a chat request returns HTTP 500 "the current context does not support
// logits computation". The admin-settings handler must therefore REJECT
// setting an embedding model as the extraction model with a 400
// (INVALID_EXTRACTION_MODEL); a chat (or any non-embedding) model is
// accepted.
//
// This is the deterministic, no-real-LLM counterpart to the runtime
// guard unit-tested in `src/modules/memory/engine/capability.rs`.
//
// The symmetric inverse — a non-embedding model set as the EMBEDDING
// model — is rejected the same way (400 INVALID_EMBEDDING_MODEL).
//
// NOTE on the runtime guard (extractor/summarizer skip): it is covered by
// the pure unit tests on `generation_unsupported_reason`. An end-to-end
// integration test of the *runtime* skip can't deterministically isolate
// "the guard skipped" from "the provider call failed" without standing up
// a stub LLM (both paths write no memory row), and the config-time 400
// here already prevents the misconfiguration from being stored via the
// API — so the runtime path is a safety net for pre-existing bad rows.
// ============================================================================

use reqwest::StatusCode;
use serde_json::{Value, json};

const ADMIN_PERMS: &[&str] = &[
    "memory::admin::read",
    "memory::admin::manage",
    "llm_providers::read",
    "llm_providers::create",
    "llm_models::read",
    "llm_models::create",
];

/// Create a disabled provider with a throwaway key — we never call the
/// LLM, we only need a `provider_id` to hang models off.
async fn create_provider(server: &crate::common::TestServer, token: &str) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "Extraction Validation Provider",
            "provider_type": "openai",
            "enabled": false,
            "api_key": "sk-test123",
        }))
        .send()
        .await
        .expect("POST /llm-providers");
    assert_eq!(res.status(), StatusCode::CREATED, "create provider failed");
    let body: Value = res.json().await.unwrap();
    body["id"].as_str().expect("provider id").to_string()
}

/// Create a model row with the given capabilities. `engine_type: none`
/// means no engine is spawned — the row exists purely for the settings
/// handler to read its `capabilities`.
async fn create_model(
    server: &crate::common::TestServer,
    token: &str,
    provider_id: &str,
    name: &str,
    capabilities: Value,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": name,
            "display_name": name,
            "description": "extraction-validation test model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": capabilities,
        }))
        .send()
        .await
        .expect("POST /llm-models");
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "create model {name} failed"
    );
    let body: Value = res.json().await.unwrap();
    body["id"].as_str().expect("model id").to_string()
}

async fn put_admin_settings(
    server: &crate::common::TestServer,
    token: &str,
    body: Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("PUT /memory/admin-settings")
}

#[tokio::test]
async fn embedding_model_rejected_as_extraction_model() {
    let server = crate::common::TestServer::start().await;
    let admin =
        crate::common::test_helpers::create_user_with_permissions(&server, "ext_val_emb", ADMIN_PERMS)
            .await;

    let provider = create_provider(&server, &admin.token).await;
    let embedding_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-embed-model",
        json!({ "text_embedding": true }),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "default_extraction_model_id": embedding_id }),
    )
    .await;

    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "setting an embedding model as the extraction model must be rejected"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["error_code"], "INVALID_EXTRACTION_MODEL",
        "expected INVALID_EXTRACTION_MODEL, body was {body}"
    );
    // The offending model name must NOT leak in the client-facing message
    // (it's logged server-side only) — the message is generic.
    assert!(
        !body["error"].as_str().unwrap_or("").contains("test-embed-model"),
        "model name must not leak in the 400 body: {body}"
    );

    // The bad value must NOT have landed.
    let after: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        after["default_extraction_model_id"].is_null(),
        "rejected extraction model must not be persisted"
    );
}

#[tokio::test]
async fn chat_model_accepted_as_extraction_model() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ext_val_chat",
        ADMIN_PERMS,
    )
    .await;

    let provider = create_provider(&server, &admin.token).await;
    let chat_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-chat-model",
        json!({ "chat": true }),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "default_extraction_model_id": chat_id }),
    )
    .await;

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a chat model must be accepted as the extraction model"
    );
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["default_extraction_model_id"], chat_id);
}

#[tokio::test]
async fn unflagged_model_accepted_as_extraction_model() {
    // A manually-added model with no capability flags set must NOT be
    // false-rejected — only embedding models are blocked.
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ext_val_unflagged",
        ADMIN_PERMS,
    )
    .await;

    let provider = create_provider(&server, &admin.token).await;
    let model_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-unflagged-model",
        json!({}),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "default_extraction_model_id": model_id }),
    )
    .await;

    assert_eq!(res.status(), StatusCode::OK, "unflagged model must be accepted");
}

#[tokio::test]
async fn dual_flagged_model_rejected_as_extraction_model() {
    // A model flagged BOTH chat and text_embedding is still rejected — the
    // local runtime starts any text_embedding model with `--embeddings`,
    // so it can't compute logits. The most counter-intuitive rule; pin it
    // at the HTTP layer (the unit test covers the predicate).
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ext_val_dual",
        ADMIN_PERMS,
    )
    .await;

    let provider = create_provider(&server, &admin.token).await;
    let dual_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-dual-model",
        json!({ "chat": true, "text_embedding": true }),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "default_extraction_model_id": dual_id }),
    )
    .await;

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error_code"], "INVALID_EXTRACTION_MODEL");
}

#[tokio::test]
async fn embedding_model_rejected_as_user_extraction_override() {
    // The per-user override (`PUT /memory/settings`, reachable by any
    // `memory::write` user — NOT an admin) is validated the same way.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ext_val_user_override",
        &[
            "memory::read",
            "memory::write",
            "llm_providers::read",
            "llm_providers::create",
            "llm_models::read",
            "llm_models::create",
        ],
    )
    .await;

    let provider = create_provider(&server, &user.token).await;
    let embedding_id = create_model(
        &server,
        &user.token,
        &provider,
        "test-user-embed-model",
        json!({ "text_embedding": true }),
    )
    .await;

    let res = reqwest::Client::new()
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "extraction_model_id": embedding_id }))
        .send()
        .await
        .expect("PUT /memory/settings");

    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "a non-admin setting an embedding model as their extraction override must be rejected"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error_code"], "INVALID_EXTRACTION_MODEL");
    // No model-name disclosure to a non-admin.
    assert!(
        !body["error"].as_str().unwrap_or("").contains("test-user-embed-model"),
        "model name must not leak to a non-admin: {body}"
    );
}

#[tokio::test]
async fn chat_model_rejected_as_embedding_model() {
    // Symmetric inverse: a non-embedding model can't be the embedding
    // model (it has no embeddings endpoint) → 400 INVALID_EMBEDDING_MODEL.
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "emb_val_chat",
        ADMIN_PERMS,
    )
    .await;

    let provider = create_provider(&server, &admin.token).await;
    let chat_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-chat-as-embed",
        json!({ "chat": true }),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "embedding_model_id": chat_id }),
    )
    .await;

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["error_code"], "INVALID_EMBEDDING_MODEL",
        "expected INVALID_EMBEDDING_MODEL, body was {body}"
    );

    // The bad value must NOT have landed.
    let after: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        after["embedding_model_id"].is_null(),
        "rejected embedding model must not be persisted"
    );
}

#[tokio::test]
async fn embedding_model_accepted_as_embedding_model() {
    // The happy path for the embedding-side guard: a text_embedding model
    // is accepted (200). Setting it kicks off a best-effort background
    // dimension-probe; the 200 returns before that runs, so the assertion
    // is deterministic regardless of the probe's outcome.
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "emb_val_ok",
        ADMIN_PERMS,
    )
    .await;

    let provider = create_provider(&server, &admin.token).await;
    let embedding_id = create_model(
        &server,
        &admin.token,
        &provider,
        "test-embed-ok",
        json!({ "text_embedding": true }),
    )
    .await;

    let res = put_admin_settings(
        &server,
        &admin.token,
        json!({ "embedding_model_id": embedding_id }),
    )
    .await;

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a text_embedding model must be accepted as the embedding model"
    );
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["embedding_model_id"], embedding_id);
}
