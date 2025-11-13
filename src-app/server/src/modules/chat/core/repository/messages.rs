// Messages repository - Refactored for junction table architecture

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{Message, MessageWithContent, EditMessageRequest, EditMessageResponse};

use super::contents::get_message_contents;

/// Create a new message and add it to a branch
/// Note: This creates the message AND the branch_messages junction record
pub async fn create_message(
    pool: &PgPool,
    branch_id: Uuid,
    role: &str,
) -> Result<Message, AppError> {
    let message_id = Uuid::new_v4();

    // Start transaction
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Create message (originated_from_id = self for new messages)
    let message = sqlx::query_as!(
        Message,
        r#"
        INSERT INTO messages (id, role, originated_from_id, edit_count)
        VALUES ($1, $2, $1, 0)
        RETURNING id, role,
                  originated_from_id as "originated_from_id!",
                  edit_count,
                  created_at as "created_at: _"
        "#,
        message_id,
        role
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Add message to branch (not a clone)
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone)
        VALUES ($1, $2, false)
        "#,
        branch_id,
        message_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(message)
}

/// Get message by ID
pub async fn get_message(pool: &PgPool, id: Uuid) -> Result<Option<Message>, AppError> {
    let message = sqlx::query_as!(
        Message,
        r#"
        SELECT id, role,
               originated_from_id as "originated_from_id!",
               edit_count,
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

/// List all messages in a branch (ordered by when they were added to branch)
/// This joins through the branch_messages junction table
pub async fn list_messages_in_branch(pool: &PgPool, branch_id: Uuid) -> Result<Vec<Message>, AppError> {
    let messages = sqlx::query_as!(
        Message,
        r#"
        SELECT m.id, m.role,
               m.originated_from_id as "originated_from_id!",
               m.edit_count,
               m.created_at as "created_at: _"
        FROM messages m
        INNER JOIN branch_messages bm ON m.id = bm.message_id
        WHERE bm.branch_id = $1
        ORDER BY bm.created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(messages)
}

/// Get conversation history (messages with content) for AI context
/// This is used for building the context for AI API calls
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

/// Create a new branch from a message (for unified streaming endpoint)
/// Clones all messages from parent branch up to (but not including) the specified message
pub async fn create_branch_from_message(
    pool: &PgPool,
    conversation_id: Uuid,
    parent_branch_id: Uuid,
    message_id: Uuid,
) -> Result<crate::modules::chat::core::models::Branch, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Verify message exists in the parent branch
    let message_created_at = sqlx::query_scalar!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        parent_branch_id,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message not in branch"))?;

    // Create new branch
    let new_branch = sqlx::query_as!(
        crate::modules::chat::core::models::Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, $2, $3)
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  created_at as "created_at: _"
        "#,
        conversation_id,
        Some(parent_branch_id),
        Some(message_id)
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Clone messages from parent branch up to (but not including) the branching message
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
        SELECT $1, message_id, true, created_at
        FROM branch_messages
        WHERE branch_id = $2 AND created_at < $3
        "#,
        new_branch.id,
        parent_branch_id,
        message_created_at
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Set new branch as active
    sqlx::query!(
        r#"
        UPDATE conversations SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        new_branch.id,
        conversation_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(new_branch)
}

/// Edit a message (creates new branch with updated message)
/// This is the key operation for edit/regenerate functionality
pub async fn edit_message(
    pool: &PgPool,
    message_id: Uuid,
    conversation_id: Uuid,
    request: EditMessageRequest,
    current_branch_id: Uuid,
) -> Result<EditMessageResponse, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // 1. Get original message
    let original = sqlx::query_as!(
        Message,
        r#"
        SELECT id, role,
               originated_from_id as "originated_from_id!",
               edit_count,
               created_at as "created_at: _"
        FROM messages
        WHERE id = $1
        "#,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message"))?;

    // Get the created_at from branch_messages for cloning cutoff
    let original_created_at = sqlx::query_scalar!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        current_branch_id,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message not in branch"))?;

    // 2. Create new branch
    let new_branch = sqlx::query_as!(
        crate::modules::chat::core::models::Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, $2, $3)
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  created_at as "created_at: _"
        "#,
        conversation_id,
        Some(current_branch_id),
        Some(message_id)
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 3. Clone messages from current branch up to (but not including) edited message
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
        SELECT $1, message_id, true, created_at
        FROM branch_messages
        WHERE branch_id = $2 AND created_at < $3
        "#,
        new_branch.id,
        current_branch_id,
        original_created_at
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 4. Create the edited message
    let new_message_id = Uuid::new_v4();
    let new_message = sqlx::query_as!(
        Message,
        r#"
        INSERT INTO messages (id, role, originated_from_id, edit_count)
        VALUES ($1, $2, $3, $4)
        RETURNING id, role,
                  originated_from_id as "originated_from_id!",
                  edit_count,
                  created_at as "created_at: _"
        "#,
        new_message_id,
        original.role,
        original.originated_from_id,  // Keep same origin
        original.edit_count  // Will be incremented later
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 5. Add message content
    let content_data = serde_json::json!({"text": request.content});
    sqlx::query!(
        r#"
        INSERT INTO message_contents (message_id, content_type, content, sequence_order)
        VALUES ($1, 'text', $2, 0)
        "#,
        new_message_id,
        content_data
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 6. Add new message to branch (not a clone)
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone)
        VALUES ($1, $2, false)
        "#,
        new_branch.id,
        new_message_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 7. Set new branch as active
    sqlx::query!(
        r#"
        UPDATE conversations SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        new_branch.id,
        conversation_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 8. Increment edit_count for all messages with same originated_from_id
    sqlx::query!(
        r#"
        UPDATE messages SET edit_count = edit_count + 1
        WHERE originated_from_id = $1
        "#,
        original.originated_from_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(EditMessageResponse {
        message: new_message,
        branch: new_branch,
    })
}

/// Delete message and all its descendants
/// Note: This only removes the junction records if message is only in one branch
/// If message is cloned to multiple branches, it won't be deleted
pub async fn delete_message_and_descendants(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    // For now, simplified implementation - just delete the message
    // The cascade will handle branch_messages
    let result = sqlx::query!(
        r#"
        DELETE FROM messages
        WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected())
}

/// Count messages in a branch
pub async fn count_messages_in_branch(pool: &PgPool, branch_id: Uuid) -> Result<i64, AppError> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM branch_messages
        WHERE branch_id = $1
        "#,
        branch_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(count)
}

/// Get all branches containing a specific message or its edits
pub async fn get_message_branches(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Vec<crate::modules::chat::core::models::Branch>, AppError> {
    // Get the originated_from_id for this message
    let originated_from_id = sqlx::query_scalar!(
        r#"
        SELECT originated_from_id as "originated_from_id!"
        FROM messages
        WHERE id = $1
        "#,
        message_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message"))?;

    // Get all branches containing any message with this originated_from_id
    let branches = sqlx::query_as!(
        crate::modules::chat::core::models::Branch,
        r#"
        SELECT DISTINCT b.id, b.conversation_id, b.parent_branch_id, b.created_from_message_id,
               b.created_at as "created_at: _"
        FROM branches b
        INNER JOIN branch_messages bm ON b.id = bm.branch_id
        INNER JOIN messages m ON bm.message_id = m.id
        WHERE m.originated_from_id = $1
          AND bm.is_clone = false
        ORDER BY b.created_at ASC
        "#,
        originated_from_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(branches)
}
