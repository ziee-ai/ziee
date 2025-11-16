//! Conversation CRUD integration tests

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Create Conversation Tests
// =====================================================

#[tokio::test]
async fn test_create_conversation_minimal() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let payload = json!({});

    let response = reqwest::Client::new()
        .post(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();

    // Verify structure
    assert!(body["id"].is_string());
    assert_eq!(body["user_id"], user.user_id);
    assert!(body["model_id"].is_null());
    assert!(body["title"].is_null());
    assert!(body["active_branch_id"].is_string(), "Should have default branch");
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

#[tokio::test]
async fn test_create_conversation_with_title() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let payload = json!({
        "title": "My Test Conversation"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["title"], "My Test Conversation");
}

#[tokio::test]
async fn test_create_conversation_with_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
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

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = json!({
        "model_id": model_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();

    super::helpers::assert_uuid_eq(&body["model_id"], model_id, "model_id");
}

#[tokio::test]
async fn test_create_conversation_with_title_and_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
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

    let model = super::helpers::get_or_create_test_model(&server, &user.token).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let payload = json!({
        "title": "Conversation with Model",
        "model_id": model_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["title"], "Conversation with Model");
    super::helpers::assert_uuid_eq(&body["model_id"], model_id, "model_id");
}

#[tokio::test]
async fn test_create_conversation_creates_default_branch() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let active_branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Verify the branch exists in the database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let branch = sqlx::query!(
        "SELECT id, conversation_id FROM branches WHERE id = $1",
        active_branch_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    pool.close().await;

    assert_eq!(branch.id, active_branch_id);
    assert_eq!(branch.conversation_id, conversation_id);
}

#[tokio::test]
async fn test_create_conversation_sets_active_branch_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;

    assert!(
        !conversation["active_branch_id"].is_null(),
        "active_branch_id should be set"
    );
    assert!(
        conversation["active_branch_id"].is_string(),
        "active_branch_id should be a string UUID"
    );
}

#[tokio::test]
async fn test_create_conversation_invalid_model_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let fake_model_id = uuid::Uuid::new_v4();

    let payload = json!({
        "model_id": fake_model_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should fail - either 400 or 404 depending on validation
    assert!(
        response.status() == StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND,
        "Expected 400 or 404 for invalid model_id, got {}",
        response.status()
    );
}

// =====================================================
// List Conversations Tests
// =====================================================

#[tokio::test]
async fn test_list_conversations_empty() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 0);
}

#[tokio::test]
async fn test_list_conversations_single() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 1);
    super::helpers::assert_uuid_eq(&conversations[0]["id"], conversation_id, "id");
}

#[tokio::test]
async fn test_list_conversations_multiple_ordered_by_created_at() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // Create 3 conversations
    let conv1 = super::helpers::create_conversation(&server, &user.token, None, Some("First")).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // Ensure different timestamps
    let conv2 = super::helpers::create_conversation(&server, &user.token, None, Some("Second")).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let conv3 = super::helpers::create_conversation(&server, &user.token, None, Some("Third")).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 3);

    // Should be ordered by created_at DESC (newest first)
    let id3 = super::helpers::parse_uuid(&conv3["id"]);
    let id2 = super::helpers::parse_uuid(&conv2["id"]);
    let id1 = super::helpers::parse_uuid(&conv1["id"]);

    super::helpers::assert_uuid_eq(&conversations[0]["id"], id3, "first (newest)");
    super::helpers::assert_uuid_eq(&conversations[1]["id"], id2, "second");
    super::helpers::assert_uuid_eq(&conversations[2]["id"], id1, "third (oldest)");
}

#[tokio::test]
async fn test_list_conversations_pagination() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // Create 5 conversations
    for i in 1..=5 {
        super::helpers::create_conversation(&server, &user.token, None, Some(&format!("Conv {}", i))).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Get first page (limit 2)
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations?per_page=2&page=1"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 2, "First page should have 2 items");

    // Get second page
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations?per_page=2&page=2"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 2, "Second page should have 2 items");
}

#[tokio::test]
async fn test_list_conversations_message_count() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 1);

    // New conversation should have message_count = 0
    assert_eq!(conversations[0]["message_count"], 0);
}

#[tokio::test]
async fn test_list_conversations_only_shows_own() {
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
        &["conversations::create", "conversations::read"],
    )
    .await;

    // User1 creates 2 conversations
    super::helpers::create_conversation(&server, &user1.token, None, Some("User1 Conv1")).await;
    super::helpers::create_conversation(&server, &user1.token, None, Some("User1 Conv2")).await;

    // User2 creates 1 conversation
    super::helpers::create_conversation(&server, &user2.token, None, Some("User2 Conv1")).await;

    // User1 should only see their own 2 conversations
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 2, "User1 should only see their own conversations");

    // User2 should only see their own 1 conversation
    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let conversations: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(conversations.len(), 1, "User2 should only see their own conversation");
}

// =====================================================
// Get Conversation Tests
// =====================================================

#[tokio::test]
async fn test_get_conversation_by_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test Conversation")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    super::helpers::assert_uuid_eq(&body["id"], conversation_id, "id");
    assert_eq!(body["title"], "Test Conversation");
}

#[tokio::test]
async fn test_get_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_conversation_invalid_uuid() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/conversations/not-a-uuid"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Update Conversation Tests
// =====================================================

#[tokio::test]
async fn test_update_conversation_title() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Original Title")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": "Updated Title"
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["title"], "Updated Title");
}

#[tokio::test]
async fn test_update_conversation_clear_title() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Title to Clear")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": null
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    assert!(body["title"].is_null());
}

#[tokio::test]
async fn test_update_conversation_title_max_length() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Original")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Title max length is 500 characters
    let long_title = "a".repeat(500);
    let payload = json!({
        "title": long_title
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test title over max length
    let too_long_title = "a".repeat(501);
    let payload = json!({
        "title": too_long_title
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();
    let payload = json!({
        "title": "Won't Work"
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/conversations/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Delete Conversation Tests
// =====================================================

#[tokio::test]
async fn test_delete_conversation_successfully() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::delete"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("To Delete")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_conversation_verifies_deletion() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::delete",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("To Delete")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Try to get it - should be 404
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_conversation_cascades_to_branches() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::delete"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("To Delete")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Delete conversation
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify branch is also deleted
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let result = sqlx::query!("SELECT id FROM branches WHERE id = $1", branch_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

    pool.close().await;

    assert!(result.is_none(), "Branch should be cascade deleted");
}

#[tokio::test]
async fn test_delete_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::delete"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/conversations/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
