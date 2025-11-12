// Branch-Messages repository - Junction table operations for copy-on-write branching

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::BranchMessage;

/// Convert chrono::DateTime<Utc> to time::OffsetDateTime for SQLx
fn to_offset_datetime(dt: chrono::DateTime<chrono::Utc>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .expect("valid timestamp")
        .replace_nanosecond(dt.timestamp_subsec_nanos())
        .expect("valid nanoseconds")
}

/// Add a message to a branch (create junction record)
pub async fn add_message_to_branch(
    pool: &PgPool,
    branch_id: Uuid,
    message_id: Uuid,
    is_clone: bool,
) -> Result<BranchMessage, AppError> {
    let branch_message = sqlx::query_as!(
        BranchMessage,
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone)
        VALUES ($1, $2, $3)
        RETURNING id, branch_id, message_id, is_clone,
                  created_at as "created_at: _"
        "#,
        branch_id,
        message_id,
        is_clone
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(branch_message)
}

/// Clone messages from one branch to another (copy-on-write)
/// Messages are referenced, not copied - they remain in their original location
pub async fn clone_messages_to_branch(
    pool: &PgPool,
    source_branch_id: Uuid,
    target_branch_id: Uuid,
    up_to_timestamp: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<Uuid>, AppError> {
    let cloned_message_ids = if let Some(timestamp) = up_to_timestamp {
        // Clone messages created before specified timestamp
        let ts = to_offset_datetime(timestamp);
        sqlx::query_scalar!(
            r#"
            INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
            SELECT $1, message_id, true, created_at
            FROM branch_messages
            WHERE branch_id = $2 AND created_at < $3
            RETURNING message_id
            "#,
            target_branch_id,
            source_branch_id,
            ts
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::database_error)?
    } else {
        // Clone all messages
        sqlx::query_scalar!(
            r#"
            INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
            SELECT $1, message_id, true, created_at
            FROM branch_messages
            WHERE branch_id = $2
            RETURNING message_id
            "#,
            target_branch_id,
            source_branch_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::database_error)?
    };

    Ok(cloned_message_ids)
}

/// Get all messages in a branch (including clones)
pub async fn get_messages_in_branch(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let message_ids = sqlx::query_scalar!(
        r#"
        SELECT message_id
        FROM branch_messages
        WHERE branch_id = $1
        ORDER BY created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(message_ids)
}

/// Get all branch_message records for a branch
pub async fn get_branch_messages(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<BranchMessage>, AppError> {
    let branch_messages = sqlx::query_as!(
        BranchMessage,
        r#"
        SELECT id, branch_id, message_id, is_clone,
               created_at as "created_at: _"
        FROM branch_messages
        WHERE branch_id = $1
        ORDER BY created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(branch_messages)
}

/// Check if a message belongs to a branch
pub async fn message_in_branch(
    pool: &PgPool,
    branch_id: Uuid,
    message_id: Uuid,
) -> Result<bool, AppError> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM branch_messages
            WHERE branch_id = $1 AND message_id = $2
        ) as "exists!"
        "#,
        branch_id,
        message_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(exists)
}

/// Remove a message from a branch (delete junction record)
/// Note: This doesn't delete the message itself, just removes it from this branch
pub async fn remove_message_from_branch(
    pool: &PgPool,
    branch_id: Uuid,
    message_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        branch_id,
        message_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected() > 0)
}
