//! Branch management integration tests

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Create Branch Tests
// =====================================================

#[tokio::test]
async fn test_create_branch_minimal() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "branches::create",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Create conversation with model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);
    let conversation = super::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send a message to create a message in the branch
    let message = super::helpers::send_message(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        "Test message",
    )
    .await;
    let message_id = super::helpers::parse_uuid(&message["id"]);

    // Now create a branch from that message
    let payload = json!({
        "from_message_id": message_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();

    assert!(body["id"].is_string());
    assert_eq!(body["conversation_id"], conversation["id"]);
}

#[tokio::test]
async fn test_create_branch_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["branches::create"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();
    let fake_message_id = uuid::Uuid::new_v4();
    let payload = json!({
        "from_message_id": fake_message_id.to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// List Branches Tests
// =====================================================

#[tokio::test]
async fn test_list_branches_default() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let branches: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(branches.len(), 1, "New conversation should have 1 default branch");
}

#[tokio::test]
async fn test_list_branches_multiple() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "conversations::read",
            "branches::create",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Create conversation with model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);
    let conversation = super::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send a message to create a message in the branch
    let message = super::helpers::send_message(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        "Test message",
    )
    .await;
    let message_id = super::helpers::parse_uuid(&message["id"]);

    // Create 2 additional branches from the same message
    super::helpers::create_branch(&server, &user.token, conversation_id, Some(message_id)).await;
    super::helpers::create_branch(&server, &user.token, conversation_id, Some(message_id)).await;

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let branches: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(branches.len(), 3, "Should have 3 branches total (1 default + 2 created)");
}

#[tokio::test]
async fn test_list_branches_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/branches", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Activate Branch Tests
// =====================================================

#[tokio::test]
async fn test_activate_branch_updates_active_branch_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "conversations::read",
            "branches::create",
            "branches::switch",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Create conversation with model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);
    let conversation = super::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let original_branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send a message to create a branching point
    let message = super::helpers::send_message(
        &server,
        &user.token,
        conversation_id,
        original_branch_id,
        model_id,
        "Test message",
    )
    .await;
    let message_id = super::helpers::parse_uuid(&message["id"]);

    // Create a new branch from that message
    let new_branch = super::helpers::create_branch(&server, &user.token, conversation_id, Some(message_id)).await;
    let new_branch_id = super::helpers::parse_uuid(&new_branch["id"]);

    // Activate the new branch
    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, new_branch_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify active_branch_id changed by fetching the conversation
    let conversation = super::helpers::get_conversation(&server, &user.token, conversation_id).await;
    super::helpers::assert_uuid_eq(&conversation["active_branch_id"], new_branch_id, "active_branch_id");
    assert!(original_branch_id != new_branch_id, "Branch should have changed");
}

#[tokio::test]
async fn test_activate_branch_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "branches::switch"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let fake_branch_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, fake_branch_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_activate_branch_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["branches::switch"],
    )
    .await;

    let fake_conversation_id = uuid::Uuid::new_v4();
    let fake_branch_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            fake_conversation_id, fake_branch_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Branch Structure Tests
// =====================================================

#[tokio::test]
async fn test_default_branch_has_no_parent() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let default_branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Check database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let branch = sqlx::query!(
        "SELECT parent_branch_id FROM branches WHERE id = $1",
        default_branch_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    pool.close().await;

    assert!(branch.parent_branch_id.is_none(), "Default branch should have no parent");
}

#[tokio::test]
async fn test_created_branch_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "branches::create",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Create conversation with model
    let model = super::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);
    let conversation = super::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send a message to create a branching point
    let message = super::helpers::send_message(
        &server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        "Test message",
    )
    .await;
    let message_id = super::helpers::parse_uuid(&message["id"]);

    let new_branch = super::helpers::create_branch(&server, &user.token, conversation_id, Some(message_id)).await;
    let new_branch_id = super::helpers::parse_uuid(&new_branch["id"]);

    // Verify branch structure
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let branch = sqlx::query!(
        "SELECT conversation_id, parent_branch_id FROM branches WHERE id = $1",
        new_branch_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    pool.close().await;

    assert_eq!(branch.conversation_id, conversation_id);
    // parent_branch_id may or may not be set depending on implementation
}

// =====================================================
// Fork Level Tests (migration 22 — fork_level column)
// =====================================================
//
// fork_level distinguishes two branching flows so the frontend can anchor the
// branch navigator at the right message after page reload:
//   - "user"      → user edited their own message
//   - "assistant" → user clicked "regenerate" on an assistant reply
//
// The column has a CHECK constraint restricting it to those two values, and
// CreateBranchRequest / SendMessageRequest default it to "user" when omitted.

async fn fork_level_test_setup(
    server: &crate::common::TestServer,
) -> (crate::common::test_helpers::TestUser, uuid::Uuid, uuid::Uuid) {
    let user = crate::common::test_helpers::create_user_with_permissions(
        server,
        "user",
        &[
            "conversations::create",
            "conversations::read",
            "branches::create",
            "messages::create",
            "messages::read",
            "messages::edit",
            "llm_models::read",
        ],
    )
    .await;

    let model = super::helpers::get_or_create_test_model(server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);
    let conversation = super::helpers::create_conversation(server, &user.token, Some(model_id), None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let message = super::helpers::send_message(
        server,
        &user.token,
        conversation_id,
        branch_id,
        model_id,
        "Test message",
    )
    .await;
    let message_id = super::helpers::parse_uuid(&message["id"]);

    (user, conversation_id, message_id)
}

#[tokio::test]
async fn test_create_branch_defaults_fork_level_user() {
    // POST /branches without a fork_level field → server defaults to "user".
    // Pins the wire-level default contract; if SendMessageRequest's
    // default_fork_level() changes, this test fails.
    let server = crate::common::TestServer::start().await;
    let (user, conversation_id, message_id) = fork_level_test_setup(&server).await;

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "from_message_id": message_id.to_string() }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["fork_level"], "user", "default fork_level should be 'user'");
}

#[tokio::test]
async fn test_create_branch_with_assistant_fork_level() {
    // Explicit fork_level=assistant → round-trips in response AND persists in DB.
    let server = crate::common::TestServer::start().await;
    let (user, conversation_id, message_id) = fork_level_test_setup(&server).await;

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "from_message_id": message_id.to_string(),
            "fork_level": "assistant",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["fork_level"], "assistant");

    // Verify persistence (response could lie; the column is what matters for reload).
    let new_branch_id = super::helpers::parse_uuid(&body["id"]);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let row = sqlx::query!(
        "SELECT fork_level FROM branches WHERE id = $1",
        new_branch_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    pool.close().await;
    assert_eq!(row.fork_level, "assistant");
}

#[tokio::test]
async fn test_edit_message_creates_user_level_branch() {
    // edit_message hardcodes fork_level='user' in the repository layer — this
    // test pins that contract so a future refactor can't silently emit
    // 'assistant' (which would scramble the frontend branch navigator).
    let server = crate::common::TestServer::start().await;
    let (user, conversation_id, message_id) = fork_level_test_setup(&server).await;

    let edited = super::helpers::edit_message(
        &server,
        &user.token,
        conversation_id,
        message_id,
        "Edited content",
    )
    .await;

    // EditMessageResponse { message, branch } — branch contains fork_level.
    assert_eq!(
        edited["branch"]["fork_level"], "user",
        "edit_message must always create a 'user' branch",
    );
}

#[tokio::test]
async fn test_create_branch_rejects_invalid_fork_level() {
    // The CHECK constraint on the branches.fork_level column rejects anything
    // outside ('user', 'assistant'). The handler doesn't pre-validate, so the
    // failure happens at INSERT time → AppError::database_error → 500.
    // Pinning the rejection (regardless of status code) prevents bad enums
    // from silently being persisted if the constraint is ever dropped.
    let server = crate::common::TestServer::start().await;
    let (user, conversation_id, message_id) = fork_level_test_setup(&server).await;

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "from_message_id": message_id.to_string(),
            "fork_level": "garbage",
        }))
        .send()
        .await
        .unwrap();

    assert!(
        response.status().is_server_error() || response.status().is_client_error(),
        "expected error status for invalid fork_level, got {}",
        response.status(),
    );
    assert_ne!(response.status(), StatusCode::CREATED);
}
