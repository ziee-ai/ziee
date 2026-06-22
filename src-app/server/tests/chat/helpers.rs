//! Helper functions for chat module integration tests

use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::chat_stream_probe::{ChatFrame, ChatStreamProbe};
use crate::common::stub_engine::StubEngine;

/// Get or create a test LLM model for chat tests
/// Returns a model with chat capability that can be used in conversations
/// Uses real AI providers (Anthropic, OpenAI, etc.) with API keys from environment
/// Creates models using admin permissions to avoid permission issues in tests
/// Model configuration for testing
pub struct TestModelConfig {
    pub provider_type: &'static str,
    pub model_name: &'static str,
    pub display_name: &'static str,
}

/// Get all test models from ai-providers crate
pub fn get_test_model_configs() -> Vec<TestModelConfig> {
    vec![
        // Anthropic models (from ai-providers/tests/test_anthropic.rs)
        TestModelConfig {
            provider_type: "anthropic",
            model_name: "claude-opus-4-1-20250805",
            display_name: "Claude Opus 4.1",
        },
        TestModelConfig {
            provider_type: "anthropic",
            model_name: "claude-sonnet-4-5-20250929",
            display_name: "Claude Sonnet 4.5",
        },
        TestModelConfig {
            provider_type: "anthropic",
            model_name: "claude-haiku-4-5-20251001",
            display_name: "Claude Haiku 4.5",
        },
        // 3.5-haiku is removed — both `-20241022` and `-latest` 404
        // for our test key. The 4.5 entry above is the current cheap
        // haiku snapshot; tests that pinned the 3.5 ID should switch
        // to it.
        // OpenAI models (from ai-providers/tests/test_openai.rs)
        TestModelConfig {
            provider_type: "openai",
            model_name: "gpt-4o",
            display_name: "GPT-4o",
        },
        TestModelConfig {
            provider_type: "openai",
            model_name: "gpt-4o-mini",
            display_name: "GPT-4o Mini",
        },
        TestModelConfig {
            provider_type: "openai",
            model_name: "gpt-4-turbo",
            display_name: "GPT-4 Turbo",
        },
        TestModelConfig {
            provider_type: "openai",
            model_name: "gpt-3.5-turbo",
            display_name: "GPT-3.5 Turbo",
        },
        // Gemini models (from ai-providers/tests/test_gemini.rs)
        TestModelConfig {
            provider_type: "gemini",
            model_name: "models/gemini-2.5-flash",
            display_name: "Gemini 2.5 Flash",
        },
        TestModelConfig {
            provider_type: "gemini",
            model_name: "models/gemini-2.5-pro",
            display_name: "Gemini 2.5 Pro",
        },
        TestModelConfig {
            provider_type: "gemini",
            model_name: "models/gemini-2.0-flash",
            display_name: "Gemini 2.0 Flash",
        },
        TestModelConfig {
            provider_type: "gemini",
            model_name: "models/gemini-2.0-flash-lite",
            display_name: "Gemini 2.0 Flash Lite",
        },
    ]
}

/// Create a specific model (used by MCP tests for multi-model testing)
/// If user_id is provided, grants access to the model through group assignments
pub async fn create_test_model_with_config(
    server: &crate::common::TestServer,
    config: &TestModelConfig,
    user_id: Option<&str>,
) -> Value {
    // Create admin user with necessary permissions for model setup
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "model_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::edit",
        ],
    )
    .await;

    let (env_var, provider_name) = match config.provider_type {
        "anthropic" => ("ANTHROPIC_API_KEY", "Anthropic"),
        "openai" => ("OPENAI_API_KEY", "OpenAI"),
        "gemini" => ("GEMINI_API_KEY", "Google Gemini"),
        "groq" => ("GROQ_API_KEY", "Groq"),
        _ => panic!("Unsupported provider type: {}", config.provider_type),
    };

    // Check if provider API key is available
    if std::env::var(env_var).is_err() {
        eprintln!("Skipping {} model '{}' - {} not set", provider_name, config.display_name, env_var);
        return json!(null);
    }

    eprintln!("Configuring provider '{}' with API key from {}", provider_name, env_var);
    let provider = configure_provider_with_api_key(server, &admin.token, provider_name, env_var).await;

    eprintln!("Creating test model '{}' for provider '{}'", config.display_name, provider_name);

    let payload = json!({
        "provider_id": provider["id"],
        "name": config.model_name,
        "display_name": config.display_name,
        "description": format!("{} model for chat testing", provider_name),
        "enabled": true,
        "engine_type": "none",
        "file_format": "gguf",  // Placeholder for API models (not actually used)
        "capabilities": {
            "chat": true,
            "completion": true,
            "embedding": false
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    let status = response.status();
    if status != StatusCode::CREATED {
        let error_body = response.text().await.unwrap();
        eprintln!("Model creation failed with status {}: {}", status, error_body);
        panic!("Failed to create test model. Status: {}, Body: {}", status, error_body);
    }

    let model = response.json().await.unwrap();
    eprintln!("Successfully created model: {}", config.display_name);

    // Grant user access if user_id provided
    if let Some(uid) = user_id {
        ensure_user_has_model_access(server, uid, &model).await;
    }

    model
}

/// Probe whether Groq's chat endpoint is actually REACHABLE + the key valid
/// from this host. Some networks/regions return a 403 "Access denied" from
/// `api.groq.com` even with a present key — in that case Groq-first must fall
/// through to a working provider rather than fail a real-LLM test. A 200/400
/// (any non-auth response) means the key + network are fine.
async fn groq_reachable() -> bool {
    let key = match std::env::var("GROQ_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return false,
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let resp = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {key}"))
        .json(&json!({
            "model": "llama-3.3-70b-versatile",
            "messages": [{ "role": "user", "content": "hi" }],
            "max_tokens": 1,
        }))
        .send()
        .await;
    match resp {
        // 401/403 → key blocked / network-denied → NOT usable.
        Ok(r) => !matches!(r.status().as_u16(), 401 | 403),
        // Network failure → treat as unreachable, fall through.
        Err(_) => false,
    }
}

/// Like `get_or_create_test_model`, but deterministically PREFERS Groq
/// (`GROQ_API_KEY` + a reachable Groq endpoint → `llama-3.3-70b-versatile`, a
/// cheap, tool-capable model) and falls back to ANTHROPIC → OPENAI → GEMINI
/// when no Groq key is present OR Groq is network-blocked from this host. The
/// created model is granted to `user_id`.
///
/// IMPORTANT — this NEVER soft-skips: if NO provider key is set at all it
/// PANICS with an actionable message. Callers that use it are asserting that a
/// real LLM run executes (per `feedback_no_ignore_unless_platform`); a missing
/// key is an environment misconfiguration (source `tests/.env.test`), not a
/// reason to silently pass.
///
/// The returned model is marked `capabilities.tools = true` so auto-attached
/// built-in MCP servers (workflow / web_search / …) actually reach the model
/// (per `project_real_llm_tool_test_capability`).
pub async fn get_or_create_groq_first_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> Value {
    // Groq-first, then the other tool-capable providers. The model names mirror
    // `get_or_create_test_model`'s per-provider defaults. `(env_var,
    // provider_name, model_name, display_name)`. Groq is only selected when its
    // endpoint is actually reachable (the key can be present but blocked).
    let (env_var, provider_name, model_name, display_name) =
        if groq_reachable().await {
            ("GROQ_API_KEY", "Groq", "llama-3.3-70b-versatile", "Llama 3.3 70B")
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            (
                "ANTHROPIC_API_KEY",
                "Anthropic",
                "claude-opus-4-1-20250805",
                "Claude Opus 4.1",
            )
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            ("OPENAI_API_KEY", "OpenAI", "gpt-4o", "GPT-4o")
        } else if std::env::var("GEMINI_API_KEY").is_ok() {
            (
                "GEMINI_API_KEY",
                "Google Gemini",
                "models/gemini-2.0-flash",
                "Gemini 2.0 Flash",
            )
        } else {
            panic!(
                "get_or_create_groq_first_model: NO usable provider API key \
                 (Groq unreachable/unset, and none of ANTHROPIC_API_KEY, \
                 OPENAI_API_KEY, GEMINI_API_KEY set). Source \
                 src-app/server/tests/.env.test before running real-LLM \
                 workflow tests — this test deliberately does NOT soft-skip."
            );
        };
    eprintln!("get_or_create_groq_first_model: selected provider '{provider_name}' ({model_name})");

    // Reuse the same provider-selection plumbing as `create_test_model_with_config`.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "groq_first_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::edit",
        ],
    )
    .await;
    let provider = configure_provider_with_api_key(server, &admin.token, provider_name, env_var).await;

    // Create the model with `capabilities.tools = true` so an auto-attached
    // built-in MCP server (workflow / web_search / …) actually reaches the model
    // (per `project_real_llm_tool_test_capability`). This is the one thing the
    // shared `create_test_model_with_config` does NOT set, so we create directly
    // here rather than through it.
    let payload = json!({
        "provider_id": provider["id"],
        "name": model_name,
        "display_name": display_name,
        "description": format!("{} model for real-LLM workflow testing", provider_name),
        "enabled": true,
        "engine_type": "none",
        "file_format": "gguf",
        "capabilities": { "chat": true, "completion": true, "embedding": false, "tools": true }
    });
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    let status = response.status();
    if status != StatusCode::CREATED {
        let error_body = response.text().await.unwrap();
        panic!(
            "get_or_create_groq_first_model: model create failed for provider \
             '{provider_name}'. Status: {status}, Body: {error_body}"
        );
    }
    let model: Value = response.json().await.unwrap();
    ensure_user_has_model_access(server, user_id, &model).await;
    model
}

pub async fn get_or_create_test_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> Value {
    // Create admin user with necessary permissions for model setup
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "model_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::edit",
        ],
    )
    .await;

    // First try to get an existing enabled model
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-models?per_page=100"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();

    if response.status() == StatusCode::OK {
        let body: Value = response.json().await.unwrap();
        if let Some(models) = body["models"].as_array() {
            // Find an enabled model suitable for chat
            for model in models {
                if model["enabled"].as_bool().unwrap_or(false) {
                    eprintln!("Using existing model: {}", model["display_name"]);
                    // Grant the user access to this existing model
                    ensure_user_has_model_access(server, user_id, model).await;
                    return model.clone();
                }
            }
        }
    }

    // No enabled model found, create one with real AI provider
    let provider = get_or_create_ai_provider(server, &admin.token).await;
    let provider_type = provider["provider_type"].as_str().unwrap();
    let provider_name = provider["name"].as_str().unwrap();

    // Determine model name and engine type based on provider
    // For MCP tool calling tests, prefer models with best tool use capabilities:
    // 1. Claude Opus 4.1 - Best at tool calling
    // 2. Claude Sonnet 4.5 - Excellent tool calling
    // 3. GPT-4o - Good tool calling
    // 4. Gemini 2.0 Flash - Native tool support
    // Use specific model if provided, otherwise use default for provider
    let (model_name, model_display_name, engine_type) = match provider_type {
        "anthropic" => ("claude-opus-4-1-20250805", "Claude Opus 4.1", "none"),
        "openai" => ("gpt-4o", "GPT-4o", "none"),
        "gemini" => ("models/gemini-2.0-flash", "Gemini 2.0 Flash", "none"),
        "groq" => ("llama-3.3-70b-versatile", "Llama 3.3 70B", "none"),
        _ => panic!("Unsupported provider type: {}", provider_type),
    };

    eprintln!("Creating test model '{}' for provider '{}'", model_display_name, provider_name);

    let payload = json!({
        "provider_id": provider["id"],
        "name": model_name,
        "display_name": model_display_name,
        "description": format!("{} model for chat testing", provider_name),
        "enabled": true,
        "engine_type": engine_type,
        "file_format": "gguf",  // Placeholder for API models (not actually used)
        "capabilities": {
            "chat": true,
            "completion": true,
            "embedding": false
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    let status = response.status();
    if status != StatusCode::CREATED {
        let error_body = response.text().await.unwrap();
        eprintln!("Model creation failed with status {}: {}", status, error_body);
        panic!("Failed to create test model. Status: {}, Body: {}", status, error_body);
    }

    let model = response.json().await.unwrap();
    eprintln!("Successfully created model: {}", model_display_name);

    // Grant the user access to this model through group assignments
    ensure_user_has_model_access(server, user_id, &model).await;

    model
}

/// Ensure a user has access to a model by setting up the group assignment chain
/// Creates: group → assigns user to group → assigns provider to group
/// This is required for the send_message access control validation
pub async fn ensure_user_has_model_access(
    server: &crate::common::TestServer,
    user_id: &str,
    model: &Value,
) {
    let provider_id = model["provider_id"].as_str().unwrap();

    // Create admin user with permissions to manage groups and providers
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "access_admin",
        &[
            "groups::create",
            "groups::assign_users",
            "llm_providers::assign_groups",
        ],
    )
    .await;

    // Create a group for this test
    let group_response = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("test_access_group_{}", &Uuid::new_v4().to_string()[..8]),
            "description": "Test group for model access",
            "permissions": []
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(group_response.status(), StatusCode::CREATED, "Failed to create group");
    let group: Value = group_response.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap();

    // Assign user to group
    let assign_user_response = reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "user_id": user_id,
            "group_id": group_id
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(assign_user_response.status(), StatusCode::NO_CONTENT, "Failed to assign user to group");

    // Assign provider to group
    let assign_provider_response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_ids": [provider_id]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(assign_provider_response.status(), StatusCode::OK, "Failed to assign provider to group");

    eprintln!("✓ User {} granted access to model {} via group {}", user_id, model["display_name"], group_id);
}

/// Configure a built-in provider with API key from environment
/// Supports: anthropic, openai, gemini, groq
async fn configure_provider_with_api_key(
    server: &crate::common::TestServer,
    token: &str,
    provider_name: &str,
    env_var: &str,
) -> Value {
    // Get all providers
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let body: Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();

    // Find the built-in provider
    let provider = providers
        .iter()
        .find(|p| p["name"].as_str() == Some(provider_name))
        .unwrap_or_else(|| panic!("Built-in provider '{}' not found", provider_name));

    let provider_id = provider["id"].as_str().unwrap();

    // Check if already configured
    if provider["enabled"].as_bool().unwrap_or(false)
        && provider["api_key"].as_str().is_some() {
        eprintln!("Provider '{}' already configured", provider_name);
        return provider.clone();
    }

    // Get API key from environment
    let api_key = std::env::var(env_var)
        .unwrap_or_else(|_| panic!("{} not set. Please source tests/.env.test", env_var));

    eprintln!("Configuring provider '{}' with API key from {}", provider_name, env_var);

    // Configure provider with API key
    let update_payload = json!({
        "enabled": true,
        "api_key": api_key
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&update_payload)
        .send()
        .await
        .unwrap();

    let status = response.status();
    if status != StatusCode::OK {
        let error_body = response.text().await.unwrap();
        panic!(
            "Failed to configure provider '{}'. Status: {}, Body: {}",
            provider_name, status, error_body
        );
    }

    response.json().await.unwrap()
}

/// Get or create an AI provider with API key for chat testing
/// Prioritizes Anthropic (Claude) as it's most reliable for tool calling tests
async fn get_or_create_ai_provider(server: &crate::common::TestServer, token: &str) -> Value {
    // Try Anthropic first (best at tool calling - Claude Opus 4.1)
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Anthropic", "ANTHROPIC_API_KEY").await;
    }

    // Fallback to OpenAI (GPT-4o has good tool calling)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "OpenAI", "OPENAI_API_KEY").await;
    }

    // Fallback to Gemini (native tool support)
    if std::env::var("GEMINI_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Google Gemini", "GEMINI_API_KEY").await;
    }

    // Fallback to Groq (OpenAI-compatible)
    if std::env::var("GROQ_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Groq", "GROQ_API_KEY").await;
    }

    panic!("No AI provider API keys found. Please set at least one in tests/.env.test");
}

/// Create a conversation with specified options
/// Returns the created conversation as JSON
pub async fn create_conversation(
    server: &crate::common::TestServer,
    token: &str,
    model_id: Option<Uuid>,
    title: Option<&str>,
) -> Value {
    let mut payload = json!({});

    if let Some(id) = model_id {
        payload["model_id"] = json!(id.to_string());
    }

    if let Some(t) = title {
        payload["title"] = json!(t);
    }

    let response = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Failed to create conversation"
    );
    response.json().await.unwrap()
}

/// Get all conversations for the user
pub async fn list_conversations(
    server: &crate::common::TestServer,
    token: &str,
) -> Value {
    let response = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Get a conversation by ID
pub async fn get_conversation(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
) -> Value {
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Update a conversation
pub async fn update_conversation(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    title: Option<&str>,
) -> Value {
    let mut payload = json!({});

    if let Some(t) = title {
        payload["title"] = json!(t);
    }

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Delete a conversation
pub async fn delete_conversation(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
) -> StatusCode {
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    response.status()
}

/// Get conversation message history
pub async fn get_conversation_history(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
) -> Value {
    let response = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/conversations/{}/messages",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Get a specific message by ID
pub async fn get_message(
    server: &crate::common::TestServer,
    token: &str,
    message_id: Uuid,
) -> Value {
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/messages/{}", message_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Edit a message (creates a new branch)
pub async fn edit_message(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    message_id: Uuid,
    new_content: &str,
) -> Value {
    let payload = json!({
        "content": new_content
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/conversations/{}/messages/{}",
            conversation_id, message_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Delete a message
pub async fn delete_message(
    server: &crate::common::TestServer,
    token: &str,
    message_id: Uuid,
) -> StatusCode {
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/messages/{}", message_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    response.status()
}

/// Create a branch from a message
pub async fn create_branch(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    from_message_id: Option<Uuid>,
) -> Value {
    let mut payload = json!({});

    if let Some(msg_id) = from_message_id {
        payload["from_message_id"] = json!(msg_id.to_string());
    }

    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

/// List all branches in a conversation
pub async fn list_branches(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
) -> Value {
    let response = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/conversations/{}/branches",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Activate a branch
pub async fn activate_branch(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
) -> Value {
    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, branch_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

/// Fire-and-forget send: POST `/conversations/{id}/messages`. Returns the raw
/// response — the body is `{userMessageId, assistantMessageId}` JSON (the reply
/// itself now streams over `GET /api/chat/stream`, NOT this response). Use
/// `send_and_collect` when you need the streamed reply.
pub async fn send_message_simple(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    model_id: Uuid,
    branch_id: Uuid,
    content: &str,
) -> reqwest::Response {
    let payload = json!({
        "content": content,
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string()
    });

    reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/messages",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap()
}

/// Send a message and return a synthetic message object carrying the assistant
/// message id (mirrors the old return shape so id-only callers don't change).
/// The `id` field is the assistant message id; `user_message_id` is also
/// included. Callers that need the assistant reply TEXT must use
/// `send_and_collect` (the reply streams asynchronously over the chat stream).
pub async fn send_message(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    content: &str,
) -> Value {
    let response = send_message_simple(
        server,
        token,
        conversation_id,
        model_id,
        branch_id,
        content,
    )
    .await;

    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "send_message (POST /messages) failed: {body}"
    );

    json!({
        "id": body["assistant_message_id"],
        "user_message_id": body["user_message_id"],
        "assistant_message_id": body["assistant_message_id"],
        "content": content,
        "conversation_id": conversation_id.to_string(),
        "branch_id": branch_id.to_string(),
    })
}

/// A fully-collected assistant turn: the persisted ids plus the reply assembled
/// from the live chat-token stream.
pub struct CollectedTurn {
    pub user_message_id: Option<Uuid>,
    pub assistant_message_id: Uuid,
    pub text: String,
    pub frames: Vec<ChatFrame>,
}

/// The faithful replacement for "send a message and read the streamed reply" in
/// the fire-and-forget model: open a chat-stream probe, subscribe to the
/// conversation BEFORE sending (so no frame is missed), POST the message, and
/// collect frames until the reply terminates. Returns the ids + assembled text.
pub async fn send_and_collect(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
    model_id: Uuid,
    content: &str,
) -> CollectedTurn {
    let mut probe = ChatStreamProbe::open(server, token).await;
    probe.subscribe(Some(conversation_id)).await;

    let response =
        send_message_simple(server, token, conversation_id, model_id, branch_id, content).await;
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "send failed: {body}");

    let assistant_message_id = parse_uuid(&body["assistant_message_id"]);
    let user_message_id = body["user_message_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok());

    let frames = probe
        .collect_until_terminal(conversation_id, std::time::Duration::from_secs(30))
        .await;
    let text = ChatStreamProbe::assemble_text(&frames);

    CollectedTurn {
        user_message_id,
        assistant_message_id,
        text,
        frames,
    }
}

/// Subscribe → POST an arbitrary `body` to `/messages` → collect the streamed
/// frames as `SSEEvent`s (the `{event, data}` shape the pre-migration
/// `parse_sse_events` returned), so existing event-name assertions
/// (`events.iter().find(|e| e.event == "mcpToolStart")`) keep working with
/// minimal call-site change. Stops at a terminal (complete/error) frame OR any
/// `stop_at` event type — pass e.g. `&["mcpApprovalRequired",
/// "mcpElicitationRequired"]` for flows that pause mid-stream awaiting a
/// separate respond/approve call. Asserts the POST returned 200.
pub async fn send_body_and_collect_events(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    body: Value,
    stop_at: &[&str],
) -> Vec<SSEEvent> {
    let mut probe = ChatStreamProbe::open(server, token).await;
    probe.subscribe(Some(conversation_id)).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let resp_body: Value = resp.json().await.unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "send body failed: {resp_body}");

    let frames = probe
        .collect_until(conversation_id, stop_at, std::time::Duration::from_secs(60))
        .await;
    frames
        .into_iter()
        .map(|f| SSEEvent {
            event: f.event_type,
            data: f.data,
        })
        .collect()
}

/// Spawn a stub-engine + create a `custom` provider pointing at it + a chat
/// model, and grant `user_id` access. KEEP the returned `StubEngine` alive for
/// the test (dropping it kills the process). Deterministic: the model replies
/// `"Hello from stub"`.
pub async fn create_stub_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> (StubEngine, Value) {
    create_stub_model_with_delay(server, user_id, 0).await
}

/// Like `create_stub_model`, but the stub paces its deltas by `chunk_delay_ms`
/// so a turn is slow enough to be observed / cancelled mid-flight.
pub async fn create_stub_model_with_delay(
    server: &crate::common::TestServer,
    user_id: &str,
    chunk_delay_ms: u64,
) -> (StubEngine, Value) {
    let stub = StubEngine::start_with_chunk_delay(chunk_delay_ms).await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "stub_model_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // A fresh `custom` (OpenAI-compatible) provider pointing at the stub. No
    // api_key is required for `custom`; loopback http passes URL validation.
    let provider_resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("Stub {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom",
            "enabled": true,
            "api_key": "test",
            "base_url": stub.base_url(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        provider_resp.status(),
        StatusCode::CREATED,
        "stub provider create failed"
    );
    let provider: Value = provider_resp.json().await.unwrap();

    let model_resp = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider["id"],
            "name": "stub-model",
            "display_name": "Stub Model",
            "description": "Deterministic stub model for chat tests",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "embedding": false }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        model_resp.status(),
        StatusCode::CREATED,
        "stub model create failed"
    );
    let model: Value = model_resp.json().await.unwrap();

    ensure_user_has_model_access(server, user_id, &model).await;

    (stub, model)
}

/// Parse SSE stream into individual chunks
/// Returns a vector of parsed JSON chunks
pub async fn parse_sse_stream(response: reqwest::Response) -> Vec<Value> {
    let bytes = response.bytes().await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    let mut chunks = Vec::new();
    for line in text.lines() {
        if let Some(json_str) = line.strip_prefix("data: ") {
            // Remove "data: " prefix
            if json_str != "[DONE]"
                && let Ok(chunk) = serde_json::from_str::<Value>(json_str) {
                    chunks.push(chunk);
                }
        }
    }
    chunks
}

/// SSE Event with event name and data
#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event: String,
    pub data: Value,
}

/// Parse SSE stream into events with their event names
/// Returns a vector of SSEEvent structs with event name and parsed data
pub async fn parse_sse_events(response: reqwest::Response) -> Vec<SSEEvent> {
    let bytes = response.bytes().await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    let mut events = Vec::new();
    let mut current_event = String::from("message"); // Default SSE event type

    for line in text.lines() {
        if line.starts_with("event: ") {
            current_event = line[7..].trim().to_string();
        } else if let Some(json_str) = line.strip_prefix("data: ") {
            // Remove "data: " prefix
            if json_str != "[DONE]"
                && let Ok(data) = serde_json::from_str::<Value>(json_str) {
                    events.push(SSEEvent {
                        event: current_event.clone(),
                        data,
                    });
                }
            // Reset to default after consuming data
            current_event = String::from("message");
        }
    }
    events
}

/// Query branch_messages junction table to verify message-branch relationships
/// Returns vector of (message_id, is_clone) tuples
pub async fn get_branch_messages(
    server: &crate::common::TestServer,
    branch_id: Uuid,
) -> Vec<(Uuid, bool)> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let rows = sqlx::query!(
        "SELECT message_id, is_clone FROM branch_messages
         WHERE branch_id = $1 ORDER BY created_at",
        branch_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    pool.close().await;

    rows.iter()
        .map(|row| (row.message_id, row.is_clone))
        .collect()
}

/// Verify branch structure matches expected message IDs and clone flags
pub async fn verify_branch_structure(
    server: &crate::common::TestServer,
    branch_id: Uuid,
    expected_message_ids: &[Uuid],
    expected_clone_flags: &[bool],
) {
    assert_eq!(
        expected_message_ids.len(),
        expected_clone_flags.len(),
        "Expected arrays must have same length"
    );

    let branch_messages = get_branch_messages(server, branch_id).await;

    assert_eq!(
        branch_messages.len(),
        expected_message_ids.len(),
        "Branch has different number of messages than expected"
    );

    for (i, (msg_id, is_clone)) in branch_messages.iter().enumerate() {
        assert_eq!(
            *msg_id, expected_message_ids[i],
            "Message ID mismatch at position {}",
            i
        );
        assert_eq!(
            *is_clone, expected_clone_flags[i],
            "Clone flag mismatch at position {}",
            i
        );
    }
}

/// Get message contents from database
/// Returns the raw message_contents rows for verification
pub async fn get_message_contents_from_db(
    server: &crate::common::TestServer,
    message_id: Uuid,
) -> Vec<Value> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let rows = sqlx::query!(
        r#"
        SELECT id, message_id, content_type, content, sequence_order, created_at
        FROM message_contents
        WHERE message_id = $1
        ORDER BY sequence_order
        "#,
        message_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    pool.close().await;

    rows.iter()
        .map(|row| {
            json!({
                "id": row.id.to_string(),
                "message_id": row.message_id.to_string(),
                "content_type": row.content_type,
                "content": row.content,
                "sequence_order": row.sequence_order,
                "created_at": row.created_at.to_string(),
            })
        })
        .collect()
}

/// Get tool use approval status from database
/// Returns None if not found, Some(status) otherwise
pub async fn get_approval_status_from_db(
    server: &crate::common::TestServer,
    tool_use_id: &str,
    branch_id: Uuid,
) -> Option<String> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let result = sqlx::query!(
        r#"
        SELECT status FROM tool_use_approvals
        WHERE tool_use_id = $1 AND branch_id = $2
        "#,
        tool_use_id,
        branch_id
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    pool.close().await;

    result.map(|row| row.status)
}

/// Get all tool use approvals for a branch from database
pub async fn get_all_approvals_from_db(
    server: &crate::common::TestServer,
    branch_id: Uuid,
) -> Vec<Value> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let rows = sqlx::query!(
        r#"
        SELECT id, tool_use_id, tool_name, status, approved_by, approval_note, created_at
        FROM tool_use_approvals
        WHERE branch_id = $1
        ORDER BY created_at
        "#,
        branch_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    pool.close().await;

    rows.iter()
        .map(|row| {
            json!({
                "id": row.id.to_string(),
                "tool_use_id": row.tool_use_id,
                "tool_name": row.tool_name,
                "status": row.status,
                "approved_by": row.approved_by.map(|u| u.to_string()),
                "approval_note": row.approval_note,
                "created_at": row.created_at.to_string(),
            })
        })
        .collect()
}

/// Extract UUIDs from JSON string fields
pub fn parse_uuid(value: &Value) -> Uuid {
    value
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("Failed to parse UUID from JSON value")
}

/// Assert that two UUIDs match (helper for cleaner test code)
pub fn assert_uuid_eq(actual: &Value, expected: Uuid, field_name: &str) {
    let actual_uuid = parse_uuid(actual);
    assert_eq!(
        actual_uuid, expected,
        "UUID mismatch for field '{}'",
        field_name
    );
}
