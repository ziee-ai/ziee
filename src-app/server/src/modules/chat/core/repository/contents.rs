// Contents repository - Handles message content blocks with delta accumulation

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{ContentBlockDelta, MessageContent, MessageContentData};

/// Create a new content block
pub async fn create_content(
    pool: &PgPool,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
    sequence_order: i32,
) -> Result<MessageContent, AppError> {
    let content_json = serde_json::to_value(&initial_data)
        .map_err(|e| AppError::database_error(e))?;

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

/// Append delta to existing content (for streaming accumulation)
/// This is the CRITICAL function for streaming - it accumulates deltas into the database
pub async fn append_delta_to_content(
    pool: &PgPool,
    content_id: Uuid,
    delta: &ContentBlockDelta,
) -> Result<(), AppError> {
    match delta {
        ContentBlockDelta::TextDelta { delta, .. } => {
            // Concatenate delta to existing text using PostgreSQL's || operator
            sqlx::query!(
                r#"
                UPDATE message_contents
                SET content = jsonb_set(
                    content,
                    '{text}',
                    to_jsonb((COALESCE(content->>'text', '')) || $1),
                    true
                ),
                updated_at = NOW()
                WHERE id = $2
                "#,
                delta,
                content_id
            )
            .execute(pool)
            .await
            .map_err(AppError::database_error)?;
        }

        ContentBlockDelta::ThinkingDelta { delta, .. } => {
            // Concatenate delta to existing thinking content
            sqlx::query!(
                r#"
                UPDATE message_contents
                SET content = jsonb_set(
                    content,
                    '{thinking}',
                    to_jsonb((COALESCE(content->>'thinking', '')) || $1),
                    true
                ),
                updated_at = NOW()
                WHERE id = $2
                "#,
                delta,
                content_id
            )
            .execute(pool)
            .await
            .map_err(AppError::database_error)?;
        }
    }

    Ok(())
}

/// Get all content blocks for a message (ordered by sequence)
pub async fn get_message_contents(pool: &PgPool, message_id: Uuid) -> Result<Vec<MessageContent>, AppError> {
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

/// Get a single content block by ID
pub async fn get_content(pool: &PgPool, id: Uuid) -> Result<Option<MessageContent>, AppError> {
    let content = sqlx::query_as!(
        MessageContent,
        r#"
        SELECT id, message_id, content_type, content, sequence_order,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM message_contents
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Update content block (for manual edits after streaming completes)
pub async fn update_content(
    pool: &PgPool,
    id: Uuid,
    data: MessageContentData,
) -> Result<Option<MessageContent>, AppError> {
    let content_json = serde_json::to_value(&data)
        .map_err(|e| AppError::database_error(e))?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        UPDATE message_contents
        SET content = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        content_json,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Delete content block
pub async fn delete_content(pool: &PgPool, id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM message_contents
        WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected() > 0)
}

/// Get content count for a message
pub async fn count_contents(pool: &PgPool, message_id: Uuid) -> Result<i64, AppError> {
    let count = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM message_contents
        WHERE message_id = $1
        "#,
        message_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(count.count.unwrap_or(0))
}
