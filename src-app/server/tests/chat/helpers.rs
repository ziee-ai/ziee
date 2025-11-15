//! Helper functions for chat module integration tests

use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

/// Get or create a test LLM model for chat tests
/// Returns a model with chat capability that can be used in conversations
pub async fn get_or_create_test_model(
    server: &crate::common::TestServer,
    token: &str,
) -> Value {
    // First try to get an existing enabled model
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-models?per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    if response.status() == StatusCode::OK {
        let body: Value = response.json().await.unwrap();
        if let Some(models) = body["models"].as_array() {
            // Find an enabled model suitable for chat
            for model in models {
                if model["enabled"].as_bool().unwrap_or(false) {
                    return model.clone();
                }
            }
        }
    }

    // No enabled model found, create one
    let provider = get_first_provider(server, token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "chat-test-model",
        "display_name": "Chat Test Model",
        "description": "A test model for chat integration tests",
        "enabled": true,  // Enable it for chat tests
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", token))
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
    response.json().await.unwrap()
}

/// Get the first available provider
async fn get_first_provider(server: &crate::common::TestServer, token: &str) -> Value {
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers?per_page=1"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let status = response.status();
    let body: Value = response.json().await.unwrap();
    eprintln!("Provider response status: {}", status);
    eprintln!("Provider response body: {}", serde_json::to_string_pretty(&body).unwrap());
    let providers = body["providers"].as_array()
        .unwrap_or_else(|| panic!("No 'providers' field in response. Body: {}", body));
    assert!(
        !providers.is_empty(),
        "No providers found - tests need at least one provider"
    );

    providers[0].clone()
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
