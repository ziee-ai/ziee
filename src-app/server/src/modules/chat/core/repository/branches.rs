// Branches repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Branch;

/// Create a new branch (for edit/regenerate functionality)
pub async fn create_branch(
    pool: &PgPool,
    conversation_id: Uuid,
    parent_branch_id: Option<Uuid>,
    created_from_message_id: Option<Uuid>,
) -> Result<Branch, AppError> {
    let branch = sqlx::query_as!(
        Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, $2, $3)
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  created_at as "created_at: _"
        "#,
        conversation_id,
        parent_branch_id,
        created_from_message_id
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

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

/// Delete a branch
pub async fn delete_branch(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM branches
        WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected())
}
