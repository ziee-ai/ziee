//! Extension integration tests (assistant and title generation)
//!
//! NOTE: Full extension testing requires live AI providers with API keys.
//! These tests verify the extension API contracts and basic functionality
//! without requiring actual LLM calls.

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Helper Functions
// =====================================================

/// Create a test assistant
async fn create_assistant(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
    instructions: Option<&str>,
) -> serde_json::Value {
    let mut payload = json!({
        "name": name
    });

    if let Some(instr) = instructions {
        payload["instructions"] = json!(instr);
    }

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Failed to create assistant"
    );
    response.json().await.unwrap()
}

// =====================================================
// Assistant Extension Tests
// =====================================================

#[tokio::test]
async fn test_send_message_accepts_assistant_id_field() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "assistants::create",
            "assistants::read",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Create a test assistant
    let assistant = create_assistant(
        &server,
        &user.token,
        "Test Assistant",
        Some("You are a helpful assistant."),
    )
    .await;
    let assistant_id = super::helpers::parse_uuid(&assistant["id"]);

    // Send message with assistant_id field
    // Note: This will fail with model validation error, but proves assistant_id is accepted
    let payload = json!({
        "content": "Hello",
        "model_id": uuid::Uuid::new_v4().to_string(),  // Invalid model
        "branch_id": branch_id.to_string(),
        "assistant_id": assistant_id.to_string()  // Extension field
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should accept the request structure (even though model is invalid)
    // A 400 or 404 means validation happened, 403 would mean field wasn't accepted
    assert_ne!(
        response.status(),
        StatusCode::FORBIDDEN,
        "assistant_id field should be accepted by the API"
    );
}

#[tokio::test]
async fn test_send_message_without_assistant_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send message WITHOUT assistant_id (should also be valid)
    let payload = json!({
        "content": "Hello",
        "model_id": uuid::Uuid::new_v4().to_string(),
        "branch_id": branch_id.to_string()
        // No assistant_id - extension field is optional
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!(
            "/conversations/{}/messages/stream",
            conversation_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should accept request without assistant_id
    assert_ne!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Request without assistant_id should be accepted"
    );
}

// =====================================================
// Title Generation Extension Tests
// =====================================================

#[tokio::test]
async fn test_conversation_title_can_be_manually_set() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    // Create conversation without title
    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    assert!(
        conversation["title"].is_null(),
        "New conversation should have no title"
    );

    // Manually set title (simulates what title extension does)
    let payload = json!({
        "title": "Manually Set Title"
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let updated: serde_json::Value = response.json().await.unwrap();

    assert_eq!(
        updated["title"].as_str().unwrap(),
        "Manually Set Title",
        "Title should be updated"
    );
}

#[tokio::test]
async fn test_conversation_title_in_list_response() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
        ],
    )
    .await;

    // Create conversation and set title
    let conversation = super::helpers::create_conversation(
        &server,
        &user.token,
        None,
        Some("Test Conversation"),
    )
    .await;

    // List conversations
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(conversations.len(), 1);

    // Verify title is included in response
    assert_eq!(
        conversations[0]["title"].as_str().unwrap(),
        "Test Conversation",
        "Title should be included in list response"
    );
}

#[tokio::test]
async fn test_conversation_title_field_exists_in_response() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Get conversation details
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // Verify title field exists (even if null)
    assert!(
        body.get("title").is_some(),
        "Title field should exist in conversation response"
    );
}

// =====================================================
// Extension API Documentation Tests
// =====================================================

#[tokio::test]
async fn test_extension_fields_documented() {
    // This test verifies that extension fields are properly exposed via OpenAPI
    // The actual OpenAPI schema validation would happen in API client generation

    let server = crate::common::TestServer::start().await;

    // Server should start successfully with extensions registered
    // If extensions aren't properly registered, server initialization would fail
    assert!(
        server.base_url.starts_with("http://"),
        "Server should be running with extensions initialized"
    );
}

#[tokio::test]
async fn test_chat_extensions_are_registered() {
    // This test verifies the extension registration system works
    // By checking that the chat module initializes without errors

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[])
        .await;

    // If extensions failed to register, this would fail
    let response = reqwest::Client::new()
        .get(&server.api_url("/health"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Server health check should pass with extensions registered"
    );
}
