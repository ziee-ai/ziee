// Conversations repository

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Conversation;
use crate::modules::chat::core::types::ConversationResponse;

/// Convert time::OffsetDateTime to chrono::DateTime<Utc>
fn to_chrono_datetime(odt: OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond()).expect("valid timestamp")
}

/// Create a new conversation with a default branch
pub async fn create_conversation(
    pool: &PgPool,
    user_id: Uuid,
    model_id: Option<Uuid>,
    title: Option<String>,
) -> Result<Conversation, AppError> {
    // Validate model_id exists if provided
    if let Some(mid) = model_id {
        let model_exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM llm_models WHERE id = $1)",
            mid
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(false);

        if !model_exists {
            return Err(AppError::not_found("Model"));
        }
    }

    // Start transaction to ensure conversation and default branch are created atomically
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // 1. Create conversation (without active_branch_id yet)
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        INSERT INTO conversations (user_id, model_id, title)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  memory_mode,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        user_id,
        model_id as Option<Uuid>,
        title
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 2. Create default branch
    let branch = sqlx::query!(
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, NULL, NULL)
        RETURNING id
        "#,
        conversation.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 3. Update conversation with active_branch_id
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        UPDATE conversations
        SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  memory_mode,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        branch.id,
        conversation.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Commit transaction
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Get conversation by ID (with user ownership check)
pub async fn get_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<Conversation>, AppError> {
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
               memory_mode,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(conversation)
}

/// List conversations for a user with pagination
pub async fn list_conversations(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationResponse>, AppError> {
    // Query without wildcard annotations since we're using query!() not query_as!()
    let conversations = sqlx::query!(
        r#"
        SELECT
            c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
            c.memory_mode, c.created_at, c.updated_at,
            COUNT(bm.message_id) as message_count
        FROM conversations c
        LEFT JOIN branches b ON b.conversation_id = c.id
        LEFT JOIN branch_messages bm ON bm.branch_id = b.id
        WHERE c.user_id = $1
        GROUP BY c.id
        ORDER BY c.updated_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let responses = conversations
        .into_iter()
        .map(|row| ConversationResponse {
            conversation: Conversation {
                id: row.id,
                user_id: row.user_id,
                model_id: row.model_id,
                title: row.title,
                active_branch_id: row.active_branch_id,
                memory_mode: row.memory_mode,
                created_at: to_chrono_datetime(row.created_at),
                updated_at: to_chrono_datetime(row.updated_at),
            },
            message_count: row.message_count.unwrap_or(0),
        })
        .collect();

    Ok(responses)
}

/// Update conversation metadata
pub async fn update_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    title: Option<Option<String>>,
) -> Result<Option<Conversation>, AppError> {
    // Handle optional updates:
    // - None = field not provided, don't update
    // - Some(None) = explicitly set to NULL
    // - Some(Some(value)) = set to value
    let conversation = match title {
        None => {
            // Don't update title, just return current conversation
            sqlx::query_as!(
                Conversation,
                r#"
                SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
                       memory_mode,
                       created_at as "created_at: _", updated_at as "updated_at: _"
                FROM conversations
                WHERE id = $1 AND user_id = $2
                "#,
                id,
                user_id
            )
            .fetch_optional(pool)
            .await
            .map_err(AppError::database_error)?
        }
        Some(new_title) => {
            // Update title (could be None for NULL or Some(value) for a string)
            sqlx::query_as!(
                Conversation,
                r#"
                UPDATE conversations
                SET
                    title = $1,
                    updated_at = NOW()
                WHERE id = $2 AND user_id = $3
                RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                          memory_mode,
                          created_at as "created_at: _", updated_at as "updated_at: _"
                "#,
                new_title as Option<String>,
                id,
                user_id
            )
            .fetch_optional(pool)
            .await
            .map_err(AppError::database_error)?
        }
    };

    Ok(conversation)
}

/// Delete conversation (cascades to branches and messages)
pub async fn delete_conversation(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected() > 0)
}

/// Update conversation model and optionally active branch
pub async fn update_conversation_state(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
    model_id: Uuid,
    branch_id: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(branch_id) = branch_id {
        // Update both model and branch
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, active_branch_id = $2, updated_at = NOW()
            WHERE id = $3 AND user_id = $4
            "#,
            model_id,
            branch_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    } else {
        // Update only model
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, updated_at = NOW()
            WHERE id = $2 AND user_id = $3
            "#,
            model_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }

    Ok(())
}
