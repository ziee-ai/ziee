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
        serde_json::to_value(&initial_data).map_err(AppError::database_error)?;

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

/// Create a new content block with an explicit UUID (used when the ID must be pre-registered,
/// e.g. elicitation rows where the registry stores the content_id before the row is inserted).
pub async fn create_content_with_id(
    pool: &PgPool,
    id: Uuid,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
    sequence_order: i32,
) -> Result<MessageContent, AppError> {
    let content_json =
        serde_json::to_value(&initial_data).map_err(AppError::database_error)?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        INSERT INTO message_contents (id, message_id, content_type, content, sequence_order)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        id,
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
        ORDER BY sequence_order ASC, created_at ASC, id ASC
        "#,
        message_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(contents)
}

/// Get all content blocks for multiple messages in a single query
/// Returns a HashMap mapping message_id -> Vec<MessageContent>
/// This is much more efficient than calling get_message_contents() N times
pub async fn get_message_contents_batch(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<MessageContent>>, AppError> {
    use std::collections::HashMap;

    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let contents = sqlx::query_as!(
        MessageContent,
        r#"
        SELECT id, message_id, content_type, content, sequence_order,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM message_contents
        WHERE message_id = ANY($1)
        ORDER BY message_id, sequence_order ASC, created_at ASC, id ASC
        "#,
        message_ids
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    // Group contents by message_id
    let mut map: HashMap<Uuid, Vec<MessageContent>> = HashMap::new();
    for content in contents {
        map.entry(content.message_id).or_default().push(content);
    }

    Ok(map)
}

/// Cancel any pending elicitation_request content blocks for the given message.
/// Called when the streaming task ends to ensure stale 'pending' rows are resolved.
pub async fn cancel_pending_elicitations(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE message_contents
        SET content = content || '{"status": "cancelled"}'::jsonb, updated_at = NOW()
        WHERE message_id = $1
          AND content_type = 'elicitation_request'
          AND content->>'status' = 'pending'
        "#,
        message_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

/// Merge JSONB fields into an existing content block (shallow merge using `||` operator).
/// Only the provided keys are updated; all other fields are preserved.
pub async fn update_content_json(
    pool: &PgPool,
    content_id: Uuid,
    patch: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE message_contents
        SET content = content || $2, updated_at = NOW()
        WHERE id = $1
        "#,
        content_id,
        patch,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}
