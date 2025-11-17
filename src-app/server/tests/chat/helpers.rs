//! Helper functions for chat module integration tests

use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

/// Get or create a test LLM model for chat tests
/// Returns a model with chat capability that can be used in conversations
/// Uses real AI providers (Anthropic, OpenAI, etc.) with API keys from environment
/// Creates models using admin permissions to avoid permission issues in tests
pub async fn get_or_create_test_model(
    server: &crate::common::TestServer,
    _token: &str,
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
        .get(&server.api_url("/llm-models?per_page=100"))
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
    let (model_name, model_display_name, engine_type) = match provider_type {
        "anthropic" => ("claude-3-5-sonnet-20240620", "Claude 3.5 Sonnet", "none"),
        "openai" => ("gpt-4o-mini", "GPT-4o Mini", "none"),
        "gemini" => ("gemini-2.0-flash-exp", "Gemini 2.0 Flash", "none"),
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
        .post(&server.api_url("/llm-models"))
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
    model
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
        .get(&server.api_url("/llm-providers?per_page=100"))
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
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
/// Prioritizes Anthropic (Claude) as it's most reliable for testing
async fn get_or_create_ai_provider(server: &crate::common::TestServer, token: &str) -> Value {
    // Try OpenAI first (most compatible)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "OpenAI", "OPENAI_API_KEY").await;
    }

    // Fallback to Groq (OpenAI-compatible, often more accessible)
    if std::env::var("GROQ_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Groq", "GROQ_API_KEY").await;
    }

    // Fallback to Gemini
    if std::env::var("GEMINI_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Google Gemini", "GEMINI_API_KEY").await;
    }

    // Fallback to Anthropic
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return configure_provider_with_api_key(server, token, "Anthropic", "ANTHROPIC_API_KEY").await;
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
        .post(&server.api_url("/conversations"))
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
        .get(&server.api_url("/conversations"))
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
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
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
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
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
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
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
        .get(&server.api_url(&format!(
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
        .get(&server.api_url(&format!("/messages/{}", message_id)))
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
        .put(&server.api_url(&format!(
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
        .delete(&server.api_url(&format!("/messages/{}", message_id)))
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
        .post(&server.api_url(&format!(
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
        .get(&server.api_url(&format!(
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
        .post(&server.api_url(&format!(
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

/// Send a message and get the streaming response
/// Note: This is a simplified version that doesn't fully parse SSE
/// For full SSE testing, use dedicated streaming tests
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
        .post(&server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap()
}

/// Send a message and return a message object with the ID
/// This is a convenience wrapper that sends a message and extracts the message ID from the stream
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

    let chunks = parse_sse_stream(response).await;

    // Find the first chunk with a message_id
    for chunk in &chunks {
        if let Some(message_id) = chunk.get("message_id") {
            if !message_id.is_null() {
                // Return a synthetic message object with the ID
                return json!({
                    "id": message_id,
                    "content": content,
                    "conversation_id": conversation_id.to_string(),
                    "branch_id": branch_id.to_string(),
                });
            }
        }
    }

    panic!("No message_id found in stream response. Chunks: {:?}", chunks);
}

/// Parse SSE stream into individual chunks
/// Returns a vector of parsed JSON chunks
pub async fn parse_sse_stream(response: reqwest::Response) -> Vec<Value> {
    let bytes = response.bytes().await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    let mut chunks = Vec::new();
    for line in text.lines() {
        if line.starts_with("data: ") {
            let json_str = &line[6..]; // Remove "data: " prefix
            if json_str != "[DONE]" {
                if let Ok(chunk) = serde_json::from_str::<Value>(json_str) {
                    chunks.push(chunk);
                }
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
        } else if line.starts_with("data: ") {
            let json_str = &line[6..]; // Remove "data: " prefix
            if json_str != "[DONE]" {
                if let Ok(data) = serde_json::from_str::<Value>(json_str) {
                    events.push(SSEEvent {
                        event: current_event.clone(),
                        data,
                    });
                }
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
