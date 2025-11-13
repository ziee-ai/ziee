// Contents repository - Handles message content blocks with delta accumulation

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{MessageContent, MessageContentData};

/// Create a new content block
pub async fn create_content(
    pool: &PgPool,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
    sequence_order: i32,
) -> Result<MessageContent, AppError> {
    let content_json =
        serde_json::to_value(&initial_data).map_err(|e| AppError::database_error(e))?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        INSERT INTO message_contents (message_id, content_type, content, sequence_order)
        VALUES ($1, $2, $3, $4)
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        message_id,
        content_type,
        content_json,
        sequence_order
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Get all content blocks for a message (ordered by sequence)
pub async fn get_message_contents(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Vec<MessageContent>, AppError> {
    let contents = sqlx::query_as!(
        MessageContent,
        r#"
        SELECT id, message_id, content_type, content, sequence_order,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM message_contents
        WHERE message_id = $1
        ORDER BY sequence_order ASC
        "#,
        message_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(contents)
}
