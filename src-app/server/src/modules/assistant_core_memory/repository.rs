//! Per-user/per-assistant core memory CRUD. All queries are scoped by
//! `user_id` — cross-user reads/writes are impossible at the SQL layer.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::CoreMemoryBlock;
use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct AssistantCoreMemoryRepository {
    pool: PgPool,
}

impl AssistantCoreMemoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_for_user_assistant(
        &self,
        user_id: Uuid,
        assistant_id: Uuid,
    ) -> Result<Vec<CoreMemoryBlock>, AppError> {
        let rows = sqlx::query_as::<_, CoreMemoryBlock>(
            r#"
            SELECT id, assistant_id, user_id, block_label, content, char_limit,
                   created_at, updated_at
            FROM assistant_core_memory
            WHERE user_id = $1 AND assistant_id = $2
            ORDER BY block_label
            "#,
        )
        .bind(user_id)
        .bind(assistant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    pub async fn upsert(
        &self,
        user_id: Uuid,
        assistant_id: Uuid,
        block_label: &str,
        content: &str,
        char_limit: i32,
    ) -> Result<CoreMemoryBlock, AppError> {
        let row = sqlx::query_as::<_, CoreMemoryBlock>(
            r#"
            INSERT INTO assistant_core_memory
                (assistant_id, user_id, block_label, content, char_limit)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (assistant_id, user_id, block_label) DO UPDATE
            SET content    = EXCLUDED.content,
                char_limit = EXCLUDED.char_limit,
                updated_at = NOW()
            RETURNING id, assistant_id, user_id, block_label, content, char_limit,
                      created_at, updated_at
            "#,
        )
        .bind(assistant_id)
        .bind(user_id)
        .bind(block_label)
        .bind(content)
        .bind(char_limit)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn delete(
        &self,
        user_id: Uuid,
        assistant_id: Uuid,
        block_label: &str,
    ) -> Result<bool, AppError> {
        let n = sqlx::query(
            "DELETE FROM assistant_core_memory WHERE user_id = $1 AND assistant_id = $2 AND block_label = $3",
        )
        .bind(user_id)
        .bind(assistant_id)
        .bind(block_label)
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(n.rows_affected() == 1)
    }
}
