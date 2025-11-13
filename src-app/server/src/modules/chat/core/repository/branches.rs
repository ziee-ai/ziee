// Branches repository

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Branch;

/// Convert time::OffsetDateTime to chrono::DateTime<Utc>
fn from_offset_datetime(odt: OffsetDateTime) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond())
        .expect("valid timestamp")
}

/// Convert chrono::DateTime<Utc> to time::OffsetDateTime for SQLx
fn to_offset_datetime(dt: chrono::DateTime<chrono::Utc>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .expect("valid timestamp")
        .replace_nanosecond(dt.timestamp_subsec_nanos())
        .expect("valid nanoseconds")
}

/// Create a new branch (for edit/regenerate functionality)
/// Clones messages from parent branch up to the specified message
/// Both parent_branch_id and created_from_message_id are required
pub async fn create_branch(
    pool: &PgPool,
    conversation_id: Uuid,
    parent_branch_id: Uuid,
    created_from_message_id: Uuid,
) -> Result<Branch, AppError> {
    // Start transaction to ensure atomicity
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // 1. Create the branch
    let branch = sqlx::query_as!(
        Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, $2, $3)
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  created_at as "created_at: _"
        "#,
        conversation_id,
        Some(parent_branch_id),
        Some(created_from_message_id)
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 2. Clone messages from parent branch up to specified message
    // Get the timestamp when the message was added to the parent branch
    let timestamp_result = sqlx::query!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        parent_branch_id,
        created_from_message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    if let Some(row) = timestamp_result {
        let up_to_timestamp = from_offset_datetime(row.created_at);
        let ts = to_offset_datetime(up_to_timestamp);

        // Clone messages created before the specified timestamp
        let _cloned_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"
            INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
            SELECT $1, message_id, true, created_at
            FROM branch_messages
            WHERE branch_id = $2 AND created_at < $3
            RETURNING message_id
            "#,
            branch.id,
            parent_branch_id,
            ts
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    // Commit transaction
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(branch)
}

/// Get branch by ID
pub async fn get_branch(pool: &PgPool, id: Uuid) -> Result<Option<Branch>, AppError> {
    let branch = sqlx::query_as!(
        Branch,
        r#"
        SELECT id, conversation_id, parent_branch_id, created_from_message_id,
               created_at as "created_at: _"
        FROM branches
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(branch)
}

/// List all branches for a conversation
pub async fn list_branches(pool: &PgPool, conversation_id: Uuid) -> Result<Vec<Branch>, AppError> {
    let branches = sqlx::query_as!(
        Branch,
        r#"
        SELECT id, conversation_id, parent_branch_id, created_from_message_id,
               created_at as "created_at: _"
        FROM branches
        WHERE conversation_id = $1
        ORDER BY created_at DESC
        "#,
        conversation_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(branches)
}

/// Set the active branch for a conversation
pub async fn set_active_branch(
    pool: &PgPool,
    conversation_id: Uuid,
    branch_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE conversations
        SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        branch_id,
        conversation_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}
