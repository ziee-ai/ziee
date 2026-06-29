//! Integration test for the "cancel still-pending elicitations" behavior the
//! chat-extension runs when a tool-execution loop ends (mcp.rs calls
//! `Repos.chat.core.cancel_pending_elicitations(message_id)` at several loop-exit
//! points so an unanswered elicitation doesn't dangle in `pending` forever).
//!
//! The wrapper is server-internal (the `modules` tree is private to the crate,
//! so it can't be invoked from an integration test), and the only other path is
//! the full real-LLM elicitation flow. This test therefore exercises the exact
//! production statement against a real Postgres to lock in its semantics:
//!   - only `elicitation_request` blocks with `status = 'pending'` flip to
//!     `'cancelled'`,
//!   - already-`accepted` elicitations and non-elicitation blocks are untouched,
//!   - elicitations on OTHER messages are not affected (it is scoped by
//!     message_id),
//!   - it is idempotent (a second run is a no-op).

use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;

/// Mirrors `chat::core::repository::contents::cancel_pending_elicitations`.
async fn cancel_pending_elicitations(pool: &sqlx::PgPool, message_id: Uuid) {
    sqlx::query(
        r#"
        UPDATE message_contents
        SET content = content || '{"status": "cancelled"}'::jsonb, updated_at = NOW()
        WHERE message_id = $1
          AND content_type = 'elicitation_request'
          AND content->>'status' = 'pending'
        "#,
    )
    .bind(message_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_message(pool: &sqlx::PgPool, user_id: Uuid) -> Uuid {
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'ec', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO messages (id, role, originated_from_id, created_at)
           VALUES ($1, 'assistant', $1, NOW())"#,
    )
    .bind(msg_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branch_messages (branch_id, message_id, created_at)
           VALUES ($1, $2, NOW())"#,
    )
    .bind(branch_id)
    .bind(msg_id)
    .execute(pool)
    .await
    .unwrap();
    msg_id
}

async fn add_content(
    pool: &sqlx::PgPool,
    message_id: Uuid,
    content_type: &str,
    content: serde_json::Value,
) -> Uuid {
    let id = Uuid::new_v4();
    // Derive the next sequence_order for this message so multiple contents on
    // the same message don't collide on the (message_id, sequence_order)
    // unique index.
    sqlx::query(
        r#"INSERT INTO message_contents (id, message_id, content_type, content, sequence_order, created_at, updated_at)
           VALUES (
               $1, $2, $3, $4,
               (SELECT COALESCE(MAX(sequence_order) + 1, 0) FROM message_contents WHERE message_id = $2),
               NOW(), NOW()
           )"#,
    )
    .bind(id)
    .bind(message_id)
    .bind(content_type)
    .bind(&content)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn status_of(pool: &sqlx::PgPool, content_id: Uuid) -> Option<String> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT content FROM message_contents WHERE id = $1")
            .bind(content_id)
            .fetch_optional(pool)
            .await
            .unwrap();
    row.and_then(|(c,)| c.get("status").and_then(|s| s.as_str().map(String::from)))
}

#[tokio::test]
async fn cancel_pending_elicitations_only_touches_pending_on_that_message() {
    let server = TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "ec_user", &[]).await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    let msg = seed_message(&pool, uid).await;
    let other_msg = seed_message(&pool, uid).await;

    // On the target message: one pending elicitation, one already accepted,
    // plus a non-elicitation block.
    let pending = add_content(
        &pool,
        msg,
        "elicitation_request",
        json!({ "status": "pending", "message": "confirm?" }),
    )
    .await;
    let accepted = add_content(
        &pool,
        msg,
        "elicitation_request",
        json!({ "status": "accepted", "message": "already answered" }),
    )
    .await;
    let text = add_content(&pool, msg, "text", json!({ "status": "pending", "text": "hi" })).await;

    // A pending elicitation on a DIFFERENT message must not be affected.
    let other_pending = add_content(
        &pool,
        other_msg,
        "elicitation_request",
        json!({ "status": "pending", "message": "other" }),
    )
    .await;

    cancel_pending_elicitations(&pool, msg).await;

    assert_eq!(status_of(&pool, pending).await.as_deref(), Some("cancelled"));
    assert_eq!(status_of(&pool, accepted).await.as_deref(), Some("accepted"));
    // A non-elicitation block keeps its status untouched.
    assert_eq!(status_of(&pool, text).await.as_deref(), Some("pending"));
    // The other message's pending elicitation is untouched (message-scoped).
    assert_eq!(
        status_of(&pool, other_pending).await.as_deref(),
        Some("pending")
    );

    // Idempotent: a second run leaves the cancelled row cancelled.
    cancel_pending_elicitations(&pool, msg).await;
    assert_eq!(status_of(&pool, pending).await.as_deref(), Some("cancelled"));

    pool.close().await;
}
