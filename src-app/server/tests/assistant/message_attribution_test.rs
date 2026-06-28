//! Integration tests for the `message_assistant` attribution table
//! (assistant chat-extension) and its read endpoint
//! `GET /api/messages/{id}/assistant`.
//!
//! Covers the previously-untested behaviors:
//!   - the attribution is **persisted in the DB** (a fresh HTTP request reads
//!     it back — i.e. it survives independently of any in-process state),
//!   - the `ON CONFLICT (message_id) DO NOTHING` insert is **idempotent**: a
//!     second insert for the same message keeps the FIRST assistant,
//!   - a message with no attribution returns `assistant_id: null`,
//!   - a non-owner gets 404 (ownership gating via the conversation chain).

use reqwest::StatusCode;
use serde_json::Value;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// Seed conversation → branch → message owned by `user_id`. Returns the message id.
async fn seed_message(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = Uuid::parse_str(user_id).unwrap();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'attr', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_branch_id = $1 WHERE id = $2")
        .bind(branch_id)
        .bind(conv_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO messages (id, role, originated_from_id, created_at)
           VALUES ($1, 'user', $1, NOW())"#,
    )
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branch_messages (branch_id, message_id, created_at)
           VALUES ($1, $2, NOW())"#,
    )
    .bind(branch_id)
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    msg_id
}

/// Production-shape attribution insert (mirrors
/// `AssistantChatRepository::insert_message_assistant`: ON CONFLICT DO NOTHING).
async fn insert_attribution(server: &TestServer, message_id: Uuid, assistant_id: Uuid) {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query(
        r#"INSERT INTO message_assistant (message_id, assistant_id)
           VALUES ($1, $2)
           ON CONFLICT (message_id) DO NOTHING"#,
    )
    .bind(message_id)
    .bind(assistant_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
}

async fn get_attribution(server: &TestServer, token: &str, message_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url(&format!("/messages/{}/assistant", message_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn message_assistant_attribution_persists_and_is_idempotent() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "attr_user", &["conversations::read"]).await;

    let msg_id = seed_message(&server, &user.user_id).await;
    let assistant_a = Uuid::new_v4();
    let assistant_b = Uuid::new_v4();

    // First attribution wins; a second insert for the same message no-ops.
    insert_attribution(&server, msg_id, assistant_a).await;
    insert_attribution(&server, msg_id, assistant_b).await;

    // Read back through the real handler (a fresh request — proves it is
    // persisted, not held in process memory).
    let resp = get_attribution(&server, &user.token, msg_id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["assistant_id"].as_str().unwrap(),
        assistant_a.to_string(),
        "ON CONFLICT DO NOTHING must keep the FIRST attributed assistant"
    );
}

#[tokio::test]
async fn message_with_no_attribution_returns_null_assistant() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "attr_none", &["conversations::read"]).await;

    let msg_id = seed_message(&server, &user.user_id).await;

    let resp = get_attribution(&server, &user.token, msg_id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(
        body["assistant_id"].is_null(),
        "a message sent without an assistant returns assistant_id: null, not 404"
    );
}

#[tokio::test]
async fn non_owner_cannot_read_message_attribution() {
    let server = TestServer::start().await;
    let owner = create_user_with_permissions(&server, "attr_owner", &["conversations::read"]).await;
    let other = create_user_with_permissions(&server, "attr_other", &["conversations::read"]).await;

    let msg_id = seed_message(&server, &owner.user_id).await;
    insert_attribution(&server, msg_id, Uuid::new_v4()).await;

    // A different user does not own the conversation chain → 404 (not a leak).
    let resp = get_attribution(&server, &other.token, msg_id).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
