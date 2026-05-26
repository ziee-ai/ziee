//! Memory repository — all SQL is user_id-scoped.
//!
//! Cross-user isolation lives at the DB layer here: every query that
//! touches `user_memories` filters `WHERE user_id = $1`. Repository
//! consumers can't accidentally leak across users because no method
//! returns rows without a user_id constraint.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{MemoryAdminSettings, UserMemory, UserMemorySettings};
use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct MemoryRepository {
    pool: PgPool,
}

impl MemoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Cheap clone of the underlying pool for callers that need to run
    /// dynamic SQL (e.g., the retriever's vector top-K, the extractor's
    /// embedding write-back) without going through a typed repo method.
    pub fn pool_clone(&self) -> PgPool {
        self.pool.clone()
    }

    // ── user_memories CRUD ──────────────────────────────────────────

    /// List own memories. Soft-deleted rows excluded.
    pub async fn list_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<UserMemory>, AppError> {
        let rows = sqlx::query_as::<_, UserMemory>(
            r#"
            SELECT id, user_id, content, embedding_model, source, source_message_id,
                   importance, confidence, kind, metadata,
                   created_at, updated_at, last_recalled_at, recall_count
            FROM user_memories
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    /// Count own memories (live).
    pub async fn count_for_user(&self, user_id: Uuid) -> Result<i64, AppError> {
        let n: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_memories WHERE user_id = $1 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(n.0)
    }

    /// Fetch a single memory, only if it belongs to `user_id`.
    pub async fn get_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
    ) -> Result<Option<UserMemory>, AppError> {
        let row = sqlx::query_as::<_, UserMemory>(
            r#"
            SELECT id, user_id, content, embedding_model, source, source_message_id,
                   importance, confidence, kind, metadata,
                   created_at, updated_at, last_recalled_at, recall_count
            FROM user_memories
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(memory_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Insert a new memory. Embedding is computed asynchronously by a
    /// background worker; this method writes `embedding=NULL`.
    pub async fn insert(
        &self,
        user_id: Uuid,
        content: &str,
        source: &str,
        importance: i16,
        kind: &str,
        metadata: &serde_json::Value,
        source_message_id: Option<Uuid>,
    ) -> Result<UserMemory, AppError> {
        let row = sqlx::query_as::<_, UserMemory>(
            r#"
            INSERT INTO user_memories
                (user_id, content, source, source_message_id, importance, kind, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, user_id, content, embedding_model, source, source_message_id,
                      importance, confidence, kind, metadata,
                      created_at, updated_at, last_recalled_at, recall_count
            "#,
        )
        .bind(user_id)
        .bind(content)
        .bind(source)
        .bind(source_message_id)
        .bind(importance)
        .bind(kind)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Update content/importance/kind/metadata on an owned memory.
    /// `WHERE user_id = $1` prevents cross-user modification.
    pub async fn update_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
        content: Option<&str>,
        importance: Option<i16>,
        kind: Option<&str>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<Option<UserMemory>, AppError> {
        let row = sqlx::query_as::<_, UserMemory>(
            r#"
            UPDATE user_memories
            SET content    = COALESCE($3, content),
                importance = COALESCE($4, importance),
                kind       = COALESCE($5, kind),
                metadata   = COALESCE($6, metadata),
                -- If content changes the embedding must be recomputed.
                embedding  = CASE WHEN $3 IS NOT NULL THEN NULL ELSE embedding END,
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            RETURNING id, user_id, content, embedding_model, source, source_message_id,
                      importance, confidence, kind, metadata,
                      created_at, updated_at, last_recalled_at, recall_count
            "#,
        )
        .bind(memory_id)
        .bind(user_id)
        .bind(content)
        .bind(importance)
        .bind(kind)
        .bind(metadata)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Soft-delete an owned memory.
    pub async fn soft_delete_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
    ) -> Result<bool, AppError> {
        let n = sqlx::query(
            r#"
            UPDATE user_memories
            SET deleted_at = NOW()
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(memory_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(n.rows_affected() == 1)
    }

    /// Hard-delete all own memories (the "forget everything" button).
    pub async fn hard_delete_all_for_user(&self, user_id: Uuid) -> Result<u64, AppError> {
        let n = sqlx::query("DELETE FROM user_memories WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        Ok(n.rows_affected())
    }

    // ── user_memory_settings ────────────────────────────────────────

    /// Fetch settings for `user_id`. Auto-creates the row with defaults
    /// (extraction OFF, retrieval OFF) on first access.
    pub async fn get_or_init_user_settings(
        &self,
        user_id: Uuid,
    ) -> Result<UserMemorySettings, AppError> {
        // Idempotent upsert. ON CONFLICT preserves existing values.
        let row = sqlx::query_as::<_, UserMemorySettings>(
            r#"
            INSERT INTO user_memory_settings (user_id) VALUES ($1)
            ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
            RETURNING user_id, extraction_enabled, retrieval_enabled, max_memories,
                      retention_days, extraction_model_id, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Update user settings (partial). NULL fields preserve.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_user_settings(
        &self,
        user_id: Uuid,
        extraction_enabled: Option<bool>,
        retrieval_enabled: Option<bool>,
        max_memories: Option<i32>,
        retention_days: Option<Option<i32>>,
        extraction_model_id: Option<Option<Uuid>>,
    ) -> Result<UserMemorySettings, AppError> {
        // Ensure the row exists first.
        let _ = self.get_or_init_user_settings(user_id).await?;

        let row = sqlx::query_as::<_, UserMemorySettings>(
            r#"
            UPDATE user_memory_settings
            SET extraction_enabled   = COALESCE($2, extraction_enabled),
                retrieval_enabled    = COALESCE($3, retrieval_enabled),
                max_memories         = COALESCE($4, max_memories),
                retention_days       = CASE WHEN $5::int IS DISTINCT FROM retention_days
                                            THEN $5
                                            ELSE retention_days END,
                extraction_model_id  = CASE WHEN $6::uuid IS DISTINCT FROM extraction_model_id
                                            THEN $6
                                            ELSE extraction_model_id END,
                updated_at = NOW()
            WHERE user_id = $1
            RETURNING user_id, extraction_enabled, retrieval_enabled, max_memories,
                      retention_days, extraction_model_id, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(extraction_enabled)
        .bind(retrieval_enabled)
        .bind(max_memories)
        .bind(retention_days.flatten())
        .bind(extraction_model_id.flatten())
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    // ── memory_admin_settings (single row, id=1) ────────────────────

    pub async fn get_admin_settings(&self) -> Result<MemoryAdminSettings, AppError> {
        let row = sqlx::query_as::<_, MemoryAdminSettings>(
            r#"
            SELECT id, embedding_model_id, embedding_dimensions,
                   default_extraction_model_id, default_top_k,
                   cosine_threshold, enabled, updated_at
            FROM memory_admin_settings
            WHERE id = 1
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn update_admin_settings(
        &self,
        embedding_model_id: Option<Option<Uuid>>,
        default_extraction_model_id: Option<Option<Uuid>>,
        default_top_k: Option<i16>,
        cosine_threshold: Option<f32>,
        enabled: Option<bool>,
    ) -> Result<MemoryAdminSettings, AppError> {
        let row = sqlx::query_as::<_, MemoryAdminSettings>(
            r#"
            UPDATE memory_admin_settings
            SET embedding_model_id          = CASE WHEN $1::uuid IS DISTINCT FROM embedding_model_id
                                                    THEN $1
                                                    ELSE embedding_model_id END,
                default_extraction_model_id = CASE WHEN $2::uuid IS DISTINCT FROM default_extraction_model_id
                                                    THEN $2
                                                    ELSE default_extraction_model_id END,
                default_top_k               = COALESCE($3, default_top_k),
                cosine_threshold            = COALESCE($4, cosine_threshold),
                enabled                     = COALESCE($5, enabled),
                updated_at                  = NOW()
            WHERE id = 1
            RETURNING id, embedding_model_id, embedding_dimensions,
                      default_extraction_model_id, default_top_k,
                      cosine_threshold, enabled, updated_at
            "#,
        )
        .bind(embedding_model_id.flatten())
        .bind(default_extraction_model_id.flatten())
        .bind(default_top_k)
        .bind(cosine_threshold)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}
