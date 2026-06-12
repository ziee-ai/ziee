//! Tier-5 helpers for summarization integration tests.
//!
//! Slimmed copy of `memory/real_llm_helpers.rs`: summarization doesn't
//! need an embedding model (no vector retrieval) so we skip the
//! Gemini provider/model registration entirely. Only Groq Llama 4 is
//! set up + registered as the deployment's `default_summarization_model_id`.
#![allow(dead_code)]

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::TestUser;

pub const GROQ_LLM_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";

#[derive(Debug, Clone)]
pub struct RealProviderIds {
    pub llm_model_id: Uuid,
}

/// Skip-gate: returns `true` (skip) when `GROQ_API_KEY` is missing.
/// Summarization's Tier-5 tests don't need an embedding provider, so
/// we don't gate on `GEMINI_API_KEY` here.
pub fn skip_if_no_keys(test_name: &str) -> bool {
    if std::env::var("GROQ_API_KEY").is_err() {
        eprintln!(
            "test {test_name} skipped: GROQ_API_KEY not set. \
             Set GROQ_API_KEY (typically via `source tests/.env.test`)."
        );
        return true;
    }
    false
}

/// Provision Groq + a Groq LLM model + set it as the deployment's
/// default summarization model. Returns the model id.
pub async fn setup_real_providers(server: &TestServer) -> RealProviderIds {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "summ_real_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
            "summarization::settings::read",
            "summarization::settings::manage",
        ],
    )
    .await;
    let token = admin.token.clone();

    let groq = configure_builtin_provider(server, &token, "Groq", "GROQ_API_KEY").await;
    let llm_model = create_model(
        server,
        &token,
        groq["id"].as_str().unwrap(),
        GROQ_LLM_MODEL,
        "Groq Llama 4 Scout",
        // `chat` is the codebase's capability flag for conversational
        // text generation — matches the round-3 chat-capability check
        // in `update_admin_settings`. An earlier draft used
        // `text_completion` which serde silently drops (the struct
        // doesn't declare it), so the model landed with `chat: None`
        // and the capability gate rejected the subsequent PUT.
        json!({ "chat": true }),
    )
    .await;
    let llm_model_id = Uuid::parse_str(llm_model["id"].as_str().unwrap()).unwrap();

    // Register as the default summarization model so the chat
    // extension's after_llm_call zero-config fallback resolves here
    // even when the test doesn't pass a model id explicitly.
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "default_summarization_model_id": llm_model_id.to_string(),
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "set default_summarization_model_id → {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );

    RealProviderIds { llm_model_id }
}

/// Look up a built-in provider by display name + flip on the
/// `<key_var>` env api key. Reads the value out of the env var so
/// individual tests don't have to.
pub async fn configure_builtin_provider(
    server: &TestServer,
    token: &str,
    name: &str,
    key_var: &str,
) -> Value {
    let key = std::env::var(key_var).expect("API key env present");
    let list: Value = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("provider {name} not preinstalled"))
        .clone();
    let provider_id = provider["id"].as_str().unwrap();
    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "api_key": key, "enabled": true }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "enable {name} provider");
    provider
}

pub async fn create_model(
    server: &TestServer,
    token: &str,
    provider_id: &str,
    name: &str,
    display_name: &str,
    capabilities: Value,
) -> Value {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": name,
            "display_name": display_name,
            "capabilities": capabilities,
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
    assert!(status.is_success(), "create_model({name}) → {status}: {body}");
    body
}

/// Create a baseline user with `conversations::*` + the debug
/// summarization-refresh permission needed to drive the test hook.
pub async fn summarization_user(server: &TestServer, name: &str) -> TestUser {
    crate::common::test_helpers::create_user_with_permissions(
        server,
        name,
        &[
            "conversations::read",
            "conversations::edit",
            "conversations::create",
            "summarization::settings::manage",
        ],
    )
    .await
}
