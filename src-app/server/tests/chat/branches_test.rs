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
        &["conversations::create", "branches::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({});

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/branches", conversation_id)))
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
    let payload = json!({});

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/conversations/{}/branches", fake_id)))
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
        .get(&server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    let branches = body["branches"].as_array().unwrap();
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Create 2 additional branches
    super::helpers::create_branch(&server, &user.token, conversation_id, None).await;
    super::helpers::create_branch(&server, &user.token, conversation_id, None).await;

    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    let branches = body["branches"].as_array().unwrap();
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
        .get(&server.api_url(&format!("/conversations/{}/branches", fake_id)))
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
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let original_branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Create a new branch
    let new_branch = super::helpers::create_branch(&server, &user.token, conversation_id, None).await;
    let new_branch_id = super::helpers::parse_uuid(&new_branch["id"]);

    // Activate the new branch
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, new_branch_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // Verify active_branch_id changed
    super::helpers::assert_uuid_eq(&body["active_branch_id"], new_branch_id, "active_branch_id");
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
        .post(&server.api_url(&format!(
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
        .post(&server.api_url(&format!(
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
        &["conversations::create", "branches::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let new_branch = super::helpers::create_branch(&server, &user.token, conversation_id, None).await;
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
