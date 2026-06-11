//! Memory repository — all SQL is user_id-scoped.
//!
//! Cross-user isolation lives at the DB layer here: every query that
//! touches `user_memories` filters `WHERE user_id = $1`. Repository
//! consumers can't accidentally leak across users because no method
//! returns rows without a user_id constraint.
//!
//! Uses `sqlx::query!` / `query_as!` / `query_scalar!` macros so SQL
//! is compile-time validated against the build DB (matches the
//! project standard — 177 macro uses vs 39 function uses across modules).

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{MemoryAdminSettings, MemoryAuditEntry, UserMemory, UserMemorySettings};
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

    /// List own memories with optional server-side filters. Soft-deleted
    /// rows excluded. Each filter is a noop when None — the `$x::text IS
    /// NULL` short-circuit lets the planner skip the predicate entirely.
    /// Search is case-insensitive substring match on `content`.
    pub async fn list_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        kind: Option<&str>,
        source: Option<&str>,
    ) -> Result<Vec<UserMemory>, AppError> {
        let rows = sqlx::query_as!(
            UserMemory,
            r#"
            SELECT
                id,
                user_id,
                content,
                embedding_model,
                source,
                source_message_id,
                importance,
                confidence,
                kind,
                metadata as "metadata: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _",
                last_recalled_at as "last_recalled_at: _",
                recall_count
            FROM user_memories
            WHERE user_id = $1
              AND deleted_at IS NULL
              AND ($4::text IS NULL OR content ILIKE '%' || $4 || '%')
              AND ($5::text IS NULL OR kind = $5)
              AND ($6::text IS NULL OR source = $6)
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            limit,
            offset,
            search,
            kind,
            source,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    /// Count own memories (live) with the same optional server-side
    /// filters as `list_for_user`. Total must respect the same predicates
    /// or the UI's `<Pagination total>` lies and you get phantom pages.
    pub async fn count_for_user(
        &self,
        user_id: Uuid,
        search: Option<&str>,
        kind: Option<&str>,
        source: Option<&str>,
    ) -> Result<i64, AppError> {
        let n = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM user_memories
            WHERE user_id = $1
              AND deleted_at IS NULL
              AND ($2::text IS NULL OR content ILIKE '%' || $2 || '%')
              AND ($3::text IS NULL OR kind = $3)
              AND ($4::text IS NULL OR source = $4)
            "#,
            user_id,
            search,
            kind,
            source,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(n)
    }

    /// Fetch a single memory, only if it belongs to `user_id`.
    pub async fn get_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
    ) -> Result<Option<UserMemory>, AppError> {
        let row = sqlx::query_as!(
            UserMemory,
            r#"
            SELECT
                id,
                user_id,
                content,
                embedding_model,
                source,
                source_message_id,
                importance,
                confidence,
                kind,
                metadata as "metadata: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _",
                last_recalled_at as "last_recalled_at: _",
                recall_count
            FROM user_memories
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            "#,
            memory_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Insert a new memory. Embedding is computed asynchronously by a
    /// background worker; this method writes `embedding=NULL`.
    /// Emits a memory_audit_log row in the same transaction so the
    /// audit trail is consistent with the data.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        user_id: Uuid,
        content: &str,
        source: &str,
        importance: i16,
        kind: &str,
        metadata: &serde_json::Value,
        source_message_id: Option<Uuid>,
        scope: &str,
        project_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Result<UserMemory, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let row = sqlx::query_as!(
            UserMemory,
            r#"
            INSERT INTO user_memories
                (user_id, content, source, source_message_id, importance, kind, metadata,
                 scope, project_id, conversation_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id,
                user_id,
                content,
                embedding_model,
                source,
                source_message_id,
                importance,
                confidence,
                kind,
                metadata as "metadata: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _",
                last_recalled_at as "last_recalled_at: _",
                recall_count
            "#,
            user_id,
            content,
            source,
            source_message_id,
            importance,
            kind,
            metadata,
            scope,
            project_id,
            conversation_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let actor_kind = match source {
            "manual" => "user",
            "mcp_tool" => "assistant",
            "extraction" => "system",
            _ => "system",
        };
        sqlx::query!(
            r#"
            INSERT INTO memory_audit_log
                (user_id, memory_id, op, source, content_snapshot, actor_kind)
            VALUES ($1, $2, 'ADD', $3, $4, $5)
            "#,
            user_id,
            row.id,
            source,
            content,
            actor_kind
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Update content/importance/kind/metadata on an owned memory.
    /// `WHERE user_id = $1` prevents cross-user modification.
    /// Audit log records the PREVIOUS content snapshot (audit R5-#2).
    pub async fn update_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
        content: Option<&str>,
        importance: Option<i16>,
        kind: Option<&str>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<Option<UserMemory>, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        let prior = sqlx::query!(
            r#"SELECT content FROM user_memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"#,
            memory_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let row = sqlx::query_as!(
            UserMemory,
            r#"
            UPDATE user_memories
            SET content    = COALESCE($3, content),
                importance = COALESCE($4, importance),
                kind       = COALESCE($5, kind),
                metadata   = COALESCE($6, metadata),
                embedding  = CASE WHEN $3::text IS NOT NULL THEN NULL ELSE embedding END,
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            RETURNING
                id,
                user_id,
                content,
                embedding_model,
                source,
                source_message_id,
                importance,
                confidence,
                kind,
                metadata as "metadata: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _",
                last_recalled_at as "last_recalled_at: _",
                recall_count
            "#,
            memory_id,
            user_id,
            content,
            importance,
            kind,
            metadata
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        if let Some(ref r) = row {
            let snapshot = prior.as_ref().map(|p| p.content.as_str()).unwrap_or(r.content.as_str());
            sqlx::query!(
                r#"
                INSERT INTO memory_audit_log
                    (user_id, memory_id, op, source, content_snapshot, actor_kind)
                VALUES ($1, $2, 'UPDATE', $3, $4, 'user')
                "#,
                user_id,
                r.id,
                r.source,
                snapshot
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Soft-delete an owned memory.
    pub async fn soft_delete_owned(
        &self,
        user_id: Uuid,
        memory_id: Uuid,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let n = sqlx::query!(
            r#"
            UPDATE user_memories
            SET deleted_at = NOW()
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
            "#,
            memory_id,
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let deleted = n.rows_affected() == 1;
        if deleted {
            sqlx::query!(
                r#"
                INSERT INTO memory_audit_log
                    (user_id, memory_id, op, source, actor_kind)
                VALUES ($1, $2, 'DELETE', 'manual', 'user')
                "#,
                user_id,
                memory_id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(deleted)
    }

    /// Hard-delete all own memories (the "forget everything" button).
    pub async fn hard_delete_all_for_user(&self, user_id: Uuid) -> Result<u64, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let n = sqlx::query!(
            "DELETE FROM user_memories WHERE user_id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let count = n.rows_affected();
        if count > 0 {
            sqlx::query!(
                r#"
                INSERT INTO memory_audit_log
                    (user_id, op, source, actor_kind, metadata)
                VALUES ($1, 'BULK_DELETE', 'manual', 'user', $2)
                "#,
                user_id,
                serde_json::json!({ "deleted_count": count })
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(count)
    }

    /// List audit-log entries for the caller. Most recent first.
    pub async fn list_audit_log(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<MemoryAuditEntry>, AppError> {
        let rows = sqlx::query_as!(
            MemoryAuditEntry,
            r#"
            SELECT
                id,
                user_id,
                memory_id,
                op,
                source,
                content_snapshot,
                actor_kind,
                metadata as "metadata: _",
                created_at as "created_at: _"
            FROM memory_audit_log
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            user_id,
            limit.clamp(1, 500)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    // ── user_memory_settings ────────────────────────────────────────

    /// Fetch settings for `user_id`. Auto-creates the row with defaults
    /// (extraction OFF, retrieval OFF) on first access.
    pub async fn get_or_init_user_settings(
        &self,
        user_id: Uuid,
    ) -> Result<UserMemorySettings, AppError> {
        let row = sqlx::query_as!(
            UserMemorySettings,
            r#"
            INSERT INTO user_memory_settings (user_id) VALUES ($1)
            ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
            RETURNING
                user_id,
                extraction_enabled,
                retrieval_enabled,
                max_memories,
                retention_days,
                extraction_model_id,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id
        )
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

        // The Option<Option<T>> pattern (None = leave unchanged,
        // Some(None) = clear to NULL, Some(Some(x)) = set to x) doesn't
        // map cleanly to the macro's COALESCE pattern. We split into
        // two flags + value to keep the SQL literal.
        let retention_days_set = retention_days.is_some();
        let retention_days_val = retention_days.flatten();
        let extraction_model_set = extraction_model_id.is_some();
        let extraction_model_val = extraction_model_id.flatten();

        let row = sqlx::query_as!(
            UserMemorySettings,
            r#"
            UPDATE user_memory_settings
            SET extraction_enabled   = COALESCE($2, extraction_enabled),
                retrieval_enabled    = COALESCE($3, retrieval_enabled),
                max_memories         = COALESCE($4, max_memories),
                retention_days       = CASE WHEN $5::bool THEN $6 ELSE retention_days END,
                extraction_model_id  = CASE WHEN $7::bool THEN $8 ELSE extraction_model_id END,
                updated_at = NOW()
            WHERE user_id = $1
            RETURNING
                user_id,
                extraction_enabled,
                retrieval_enabled,
                max_memories,
                retention_days,
                extraction_model_id,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id,
            extraction_enabled,
            retrieval_enabled,
            max_memories,
            retention_days_set,
            retention_days_val,
            extraction_model_set,
            extraction_model_val
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    // ── memory_admin_settings (single row, id=1) ────────────────────

    pub async fn get_admin_settings(&self) -> Result<MemoryAdminSettings, AppError> {
        let row = sqlx::query_as!(
            MemoryAdminSettings,
            r#"
            SELECT
                id,
                embedding_model_id,
                embedding_dimensions,
                default_extraction_model_id,
                default_top_k,
                cosine_threshold,
                enabled,
                soft_delete_grace_days,
                daily_extraction_quota,
                summarize_after_tokens,
                summarizer_keep_recent_tokens,
                full_summary_prompt,
                incremental_summary_prompt,
                fts_dictionary,
                fts_enabled,
                fts_rrf_k,
                fts_candidate_multiplier,
                fts_min_rank,
                fts_rebuild_started_at as "fts_rebuild_started_at: _",
                fts_rebuild_completed_at as "fts_rebuild_completed_at: _",
                semantic_enabled,
                updated_at as "updated_at: _"
            FROM memory_admin_settings
            WHERE id = 1
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Atomically claim the FTS-rebuild slot. Returns Ok(true) if this
    /// caller now owns the slot (and must run the rebuild + call
    /// `clear_fts_rebuild_marker` on any error path), Ok(false) if another
    /// rebuild is already in flight.
    ///
    /// "In flight" = `fts_rebuild_started_at IS NOT NULL AND
    /// fts_rebuild_completed_at IS NULL`. The CAS is a single UPDATE
    /// keyed on that predicate, so two concurrent callers can't both
    /// observe "no rebuild in flight" and race to start one.
    pub async fn try_claim_fts_rebuild(&self) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "UPDATE memory_admin_settings
             SET fts_rebuild_started_at = NOW(),
                 fts_rebuild_completed_at = NULL
             WHERE id = 1
               AND (fts_rebuild_started_at IS NULL
                    OR fts_rebuild_completed_at IS NOT NULL)"
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(result.rows_affected() > 0)
    }

    /// Reset the started-at marker when a rebuild failed. Idempotent —
    /// always returns Ok even if a row didn't match. Called from the
    /// error path of `trigger_fts_rebuild`'s spawned worker so a crash
    /// or DDL failure doesn't leave the row permanently "in progress".
    pub async fn clear_fts_rebuild_marker(&self) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE memory_admin_settings
             SET fts_rebuild_started_at = NULL,
                 fts_rebuild_completed_at = NULL
             WHERE id = 1
               AND fts_rebuild_completed_at IS NULL"
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Mark the FTS rebuild as complete and persist the new dictionary
    /// value. Runs INSIDE the same transaction as the DDL so a failed
    /// rebuild rolls back the dictionary swap and the completed-at write.
    pub async fn complete_fts_rebuild(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        dictionary: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE memory_admin_settings
             SET fts_dictionary = $1,
                 fts_rebuild_completed_at = NOW()
             WHERE id = 1",
            dictionary
        )
        .execute(&mut **tx)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_admin_settings(
        &self,
        embedding_model_id: Option<Option<Uuid>>,
        default_extraction_model_id: Option<Option<Uuid>>,
        default_top_k: Option<i16>,
        cosine_threshold: Option<f32>,
        enabled: Option<bool>,
        soft_delete_grace_days: Option<i32>,
        daily_extraction_quota: Option<i32>,
        summarize_after_tokens: Option<i32>,
        summarizer_keep_recent_tokens: Option<i32>,
        full_summary_prompt: Option<Option<String>>,
        incremental_summary_prompt: Option<Option<String>>,
        // FTS knobs (migration 89). `fts_dictionary` is intentionally
        // NOT writeable through this path — the handler returns 409
        // FTS_REBUILD_REQUIRED on a dictionary change and the rebuild
        // endpoint owns the swap.
        fts_enabled: Option<bool>,
        fts_rrf_k: Option<i32>,
        fts_candidate_multiplier: Option<i32>,
        fts_min_rank: Option<f32>,
        // Semantic-arm kill switch (migration 90). Mirrors fts_enabled.
        semantic_enabled: Option<bool>,
    ) -> Result<MemoryAdminSettings, AppError> {
        // Same Option<Option<T>> split as update_user_settings.
        let embedding_set = embedding_model_id.is_some();
        let embedding_val = embedding_model_id.flatten();
        let extraction_set = default_extraction_model_id.is_some();
        let extraction_val = default_extraction_model_id.flatten();
        let full_prompt_set = full_summary_prompt.is_some();
        let full_prompt_val = full_summary_prompt.flatten();
        let inc_prompt_set = incremental_summary_prompt.is_some();
        let inc_prompt_val = incremental_summary_prompt.flatten();

        let row = sqlx::query_as!(
            MemoryAdminSettings,
            r#"
            UPDATE memory_admin_settings
            SET embedding_model_id            = CASE WHEN $1::bool THEN $2 ELSE embedding_model_id END,
                default_extraction_model_id   = CASE WHEN $3::bool THEN $4 ELSE default_extraction_model_id END,
                default_top_k                 = COALESCE($5, default_top_k),
                cosine_threshold              = COALESCE($6, cosine_threshold),
                enabled                       = COALESCE($7, enabled),
                soft_delete_grace_days        = COALESCE($8, soft_delete_grace_days),
                daily_extraction_quota        = COALESCE($9, daily_extraction_quota),
                summarize_after_tokens        = COALESCE($10, summarize_after_tokens),
                summarizer_keep_recent_tokens = COALESCE($11, summarizer_keep_recent_tokens),
                full_summary_prompt           = CASE WHEN $12::bool THEN $13 ELSE full_summary_prompt END,
                incremental_summary_prompt    = CASE WHEN $14::bool THEN $15 ELSE incremental_summary_prompt END,
                fts_enabled                   = COALESCE($16, fts_enabled),
                fts_rrf_k                     = COALESCE($17, fts_rrf_k),
                fts_candidate_multiplier      = COALESCE($18, fts_candidate_multiplier),
                fts_min_rank                  = COALESCE($19, fts_min_rank),
                semantic_enabled              = COALESCE($20, semantic_enabled),
                updated_at                    = NOW()
            WHERE id = 1
            RETURNING
                id,
                embedding_model_id,
                embedding_dimensions,
                default_extraction_model_id,
                default_top_k,
                cosine_threshold,
                enabled,
                soft_delete_grace_days,
                daily_extraction_quota,
                summarize_after_tokens,
                summarizer_keep_recent_tokens,
                full_summary_prompt,
                incremental_summary_prompt,
                fts_dictionary,
                fts_enabled,
                fts_rrf_k,
                fts_candidate_multiplier,
                fts_min_rank,
                fts_rebuild_started_at as "fts_rebuild_started_at: _",
                fts_rebuild_completed_at as "fts_rebuild_completed_at: _",
                semantic_enabled,
                updated_at as "updated_at: _"
            "#,
            embedding_set,
            embedding_val,
            extraction_set,
            extraction_val,
            default_top_k,
            cosine_threshold,
            enabled,
            soft_delete_grace_days,
            daily_extraction_quota,
            summarize_after_tokens,
            summarizer_keep_recent_tokens,
            full_prompt_set,
            full_prompt_val,
            inc_prompt_set,
            inc_prompt_val,
            fts_enabled,
            fts_rrf_k,
            fts_candidate_multiplier,
            fts_min_rank,
            semantic_enabled
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}
