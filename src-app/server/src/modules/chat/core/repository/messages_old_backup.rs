// Messages repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{Message, MessageWithContent};

use super::contents::get_message_contents;

/// Create a new message
pub async fn create_message(
    pool: &PgPool,
    branch_id: Uuid,
    role: &str,
    parent_id: Option<Uuid>,
) -> Result<Message, AppError> {
    // Get next sequence number
    let max_seq = sqlx::query!(
        r#"
        SELECT COALESCE(MAX(sequence_number), -1) as max_seq
        FROM messages
        WHERE branch_id = $1
        "#,
        branch_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    let sequence_number = max_seq.max_seq.unwrap_or(-1) + 1;

    let message = sqlx::query_as!(
        Message,
        r#"
        INSERT INTO messages (branch_id, role, parent_id, sequence_number)
        VALUES ($1, $2, $3, $4)
        RETURNING id, branch_id, role, parent_id, sequence_number,
                  created_at as "created_at: _"
        "#,
        branch_id,
        role,
        parent_id,
        sequence_number
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(message)
}

/// Get message by ID
pub async fn get_message(pool: &PgPool, id: Uuid) -> Result<Option<Message>, AppError> {
    let message = sqlx::query_as!(
        Message,
        r#"
        SELECT id, branch_id, role, parent_id, sequence_number,
               created_at as "created_at: _"
        FROM messages
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(message)
}

/// Get message with all its content blocks
pub async fn get_message_with_content(pool: &PgPool, id: Uuid) -> Result<Option<MessageWithContent>, AppError> {
    let message = get_message(pool, id).await?;

    match message {
        Some(msg) => {
            let contents = get_message_contents(pool, msg.id).await?;
            Ok(Some(MessageWithContent {
                message: msg,
                contents,
            }))
        }
        None => Ok(None),
    }
}

/// List all messages in a branch (in sequence order)
pub async fn list_messages_in_branch(pool: &PgPool, branch_id: Uuid) -> Result<Vec<Message>, AppError> {
    let messages = sqlx::query_as!(
        Message,
        r#"
        SELECT id, branch_id, role, parent_id, sequence_number,
               created_at as "created_at: _"
        FROM messages
        WHERE branch_id = $1
        ORDER BY sequence_number ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(messages)
}

/// Get conversation history (messages with content) for AI context
pub async fn get_conversation_history(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<MessageWithContent>, AppError> {
    let messages = list_messages_in_branch(pool, branch_id).await?;

    let mut history = Vec::new();
    for message in messages {
        let contents = get_message_contents(pool, message.id).await?;
        history.push(MessageWithContent {
            message,
            contents,
        });
    }

    Ok(history)
}

/// Delete message and all descendants (cascades to message_contents)
pub async fn delete_message_and_descendants(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    // First, find all descendant messages recursively
    let descendants = sqlx::query!(
        r#"
        WITH RECURSIVE descendants AS (
            SELECT id FROM messages WHERE id = $1
            UNION ALL
            SELECT m.id
            FROM messages m
            INNER JOIN descendants d ON m.parent_id = d.id
        )
        SELECT id FROM descendants
        "#,
        id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let message_ids: Vec<Uuid> = descendants.iter().filter_map(|r| r.id).collect();

    // Delete all found messages (cascades to contents)
    let result = sqlx::query!(
        r#"
        DELETE FROM messages
        WHERE id = ANY($1)
        "#,
        &message_ids
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected())
}

/// Count messages in a branch
pub async fn count_messages_in_branch(pool: &PgPool, branch_id: Uuid) -> Result<i64, AppError> {
    let count = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM messages
        WHERE branch_id = $1
        "#,
        branch_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(count.count.unwrap_or(0))
}
