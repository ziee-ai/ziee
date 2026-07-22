//! Tier 4 (real LLM): a REAL Anthropic model actually decides to invoke the
//! `web_search` tool and answers from the result — the feature's headline goal.
//!
//! Gated on `ANTHROPIC_API_KEY` (skips with a message if unset; not `#[ignore]`).
//! Only the LLM is real — the search provider is a loopback mock returning a
//! unique marker, so NO real SearXNG/Brave key is needed and the assertion is
//! deterministic (the model can only know the marker by calling the tool).
//!
//! Run: `source tests/.env.test && cargo test --test integration_tests \
//!   web_search::real_llm -- --test-threads=1`

use std::sync::atomic::Ordering;

use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

#[tokio::test]
async fn real_llm_invokes_web_search_and_uses_result() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!(
            "real_llm_invokes_web_search_and_uses_result skipped: ANTHROPIC_API_KEY not set"
        );
        return;
    };

    const MARKER: &str = "ZIEEMARKER42";
    // Mock SearXNG returns the marker + counts hits; the model can only learn
    // the marker by actually calling web_search.
    let (searxng, hits) = crate::web_search::start_marker_searxng(MARKER).await;

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("ANTHROPIC_API_KEY".to_string(), api_key)],
        ..Default::default()
    })
    .await;

    // User who can configure web search (admin) + send chat (model access is
    // granted by get_or_create_test_model); web_search::use comes via Users group.
    let user = create_user_with_permissions(
        &server,
        "ws_real_llm",
        &[
            "web_search::admin::read",
            "web_search::admin::manage",
            "llm_models::read",
            "llm_providers::read",
        ],
    )
    .await;

    // A real Anthropic model flagged `capabilities.tools = true`. We create it
    // explicitly rather than via `get_or_create_test_model` (which omits the
    // tools capability): the web_search chat extension only sets its auto-attach
    // flag for tool-capable models, and the curated `model_registry` catalog
    // does not list claude-opus-4-1, so without an explicit capability the
    // tools would never reach the model and it would hallucinate the call.
    let model = create_tool_capable_anthropic_model(&server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    // Configure web_search → the loopback mock, enabled.
    let client = reqwest::Client::new();
    let r = client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "config": { "base_url": searxng } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "enabled": true, "provider_chain": ["searxng"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Conversation on the real model.
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        Some("web search real-llm"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();

    // A prompt that can only be answered by calling web_search.
    let turn = crate::chat::helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "Use the web_search tool to search for 'ziee release status'. The top \
         result contains a unique status code. Reply with ONLY that exact code.",
    )
    .await;

    // The model actually invoked web_search (the mock recorded a hit) ...
    assert!(
        hits.load(Ordering::SeqCst) >= 1,
        "the real model must invoke web_search; reply was: {}",
        turn.text
    );
    // ... and used the tool's result in its answer (true end-to-end).
    assert!(
        turn.text.contains(MARKER),
        "answer must reflect the searched result marker {MARKER}; reply was: {}",
        turn.text
    );
}

/// Configure the built-in Anthropic provider with the test key and create a
/// chat model flagged `capabilities.tools = true`, granting `user_id` access.
/// The tools flag is the load-bearing bit: the web_search chat extension's
/// auto-attach only fires for tool-capable models.
async fn create_tool_capable_anthropic_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> serde_json::Value {
    use serde_json::json;
    let admin = create_user_with_permissions(
        server,
        "ws_llm_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
        ],
    )
    .await;

    // Find + configure the built-in Anthropic provider.
    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_id = body["providers"]
        .as_array()
        .expect("providers array")
        .iter()
        .find(|p| p["name"].as_str() == Some("Anthropic"))
        .expect("built-in Anthropic provider")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    // Redirect at the local LLM bridge (ANTHROPIC_BASE_URL / ZIEE_TEST_LLM_BASE_URL)
    // — else the provider hits real api.anthropic.com with a placeholder key.
    let mut provider_payload = json!({ "enabled": true, "api_key": key });
    if let Some(base_url) = crate::chat::helpers::test_provider_base_url("ANTHROPIC_API_KEY") {
        provider_payload["base_url"] = json!(base_url);
    }
    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&provider_payload)
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "configure Anthropic provider → {}",
        r.status()
    );

    // Create the model with the tools capability set.
    let r = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "claude-opus-4-1-20250805",
            "display_name": "Claude Opus 4.1 (web_search tools)",
            "description": "web_search Tier-5 tool-capable model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true }
        }))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let model: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        status,
        reqwest::StatusCode::CREATED,
        "create model → {status}: {model}"
    );

    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}
