//! Cross-user ownership and access control tests

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Conversation Ownership Tests
// =====================================================

#[tokio::test]
async fn test_user_cannot_get_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::read"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, Some("User1's Conversation")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User2 tries to get it
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_update_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "conversations::edit"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::edit"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, Some("Original")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": "Hacked Title"
    });

    // User2 tries to update it
    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_delete_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "conversations::delete"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::delete"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, Some("Protected")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User2 tries to delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_see_other_users_conversation_in_list() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::read"],
    )
    .await;

    // User1 creates a conversation
    super::helpers::create_conversation(&server, &user1.token, None, Some("User1's Conversation")).await;

    // User2 lists conversations - should not see User1's
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(conversations.len(), 0, "User2 should not see User1's conversation");
}

// =====================================================
// Message Ownership Tests
// =====================================================

#[tokio::test]
async fn test_user_cannot_get_other_users_messages() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "messages::read"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["messages::read"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User2 tries to get messages
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_send_to_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &[
            "conversations::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &[
            "messages::create",
            "llm_models::read",
        ],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let model = super::helpers::get_or_create_test_model(&server, &user1.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = json!({
        "content": "Unauthorized message",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string()
    });

    // User2 tries to send a message
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/messages/stream", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Branch Ownership Tests
// =====================================================

#[tokio::test]
async fn test_user_cannot_create_branch_in_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "branches::create"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["branches::create"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Use a fake but valid UUID for from_message_id
    let fake_message_id = uuid::Uuid::new_v4();
    let payload = json!({
        "from_message_id": fake_message_id.to_string()
    });

    // User2 tries to create a branch
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_list_other_users_branches() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::read"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User2 tries to list branches
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_activate_other_users_branch() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::create", "branches::switch"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["branches::switch"],
    )
    .await;

    // User1 creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user1.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // User2 tries to activate branch
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, branch_id
        )))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Own Resource Access Tests (verify users CAN access their own)
// =====================================================

#[tokio::test]
async fn test_user_can_access_own_conversation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // User creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("My Conversation")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User gets their own conversation
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["title"], "My Conversation");
}

#[tokio::test]
async fn test_user_can_update_own_conversation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    // User creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Original")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": "Updated"
    });

    // User updates their own conversation
    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["title"], "Updated");
}

#[tokio::test]
async fn test_user_can_delete_own_conversation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::delete"],
    )
    .await;

    // User creates a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("To Delete")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User deletes their own conversation
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
