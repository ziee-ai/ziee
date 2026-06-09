//! Message operation integration tests

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Get Conversation History Tests
// =====================================================

#[tokio::test]
async fn test_get_conversation_history_empty() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let messages: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(messages.len(), 0, "New conversation should have no messages");
}

#[tokio::test]
async fn test_get_conversation_history_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Get Message Tests
// =====================================================

#[tokio::test]
async fn test_get_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/messages/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_message_invalid_uuid() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/messages/not-a-uuid"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Edit Message Tests
// =====================================================

#[tokio::test]
async fn test_edit_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::create"],
    )
    .await;

    let fake_conversation_id = uuid::Uuid::new_v4();
    let fake_message_id = uuid::Uuid::new_v4();

    let payload = json!({
        "content": "Edited content"
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/conversations/{}/messages/{}",
            fake_conversation_id, fake_message_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_edit_message_empty_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Use a fake message ID
    let fake_message_id = uuid::Uuid::new_v4();

    let payload = json!({
        "content": ""
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/conversations/{}/messages/{}",
            conversation_id, fake_message_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should fail validation before trying to find the message
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Delete Message Tests
// =====================================================

#[tokio::test]
async fn test_delete_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::delete"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/messages/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Send Message Validation Tests
// =====================================================

#[tokio::test]
async fn test_send_message_empty_content_accepted_for_tool_only_calls() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "",
    )
    .await;

    // Empty content is now accepted by design: tool-only calls (a
    // model that issues only `tool_use` blocks with no preceding text)
    // are valid in modern LLM APIs. The fire-and-forget endpoint
    // returns 200 + `{user_message_id, assistant_message_id}`; the reply
    // itself streams over `GET /api/chat/stream`. Previously this
    // returned 400; the validation was removed when tool-only chats
    // became first-class.
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_send_message_invalid_branch_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let fake_branch_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        fake_branch_id,
        "Test message",
    )
    .await;

    // Should return 404 for non-existent branch
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_invalid_model_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let fake_model_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        fake_model_id,
        branch_id,
        "Test message",
    )
    .await;

    // Should return 404 for non-existent model
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::create"],
    )
    .await;

    let fake_conversation_id = uuid::Uuid::new_v4();
    let fake_model_id = uuid::Uuid::new_v4();
    let fake_branch_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        fake_conversation_id,
        fake_model_id,
        fake_branch_id,
        "Test message",
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_returns_message_ids() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello, world!",
    )
    .await;

    // Fire-and-forget: POST returns 200 + JSON `{user_message_id,
    // assistant_message_id}` immediately (NO body stream). The reply
    // itself streams over `GET /api/chat/stream`.
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        !body["assistant_message_id"].is_null(),
        "response body must carry a non-null assistant_message_id; got {body}"
    );
}
