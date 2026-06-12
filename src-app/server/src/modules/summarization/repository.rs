//! Singleton admin-settings repository for the summarization module.
//!
//! Per-conversation summarization-mode lives in
//! `chat_extension/repository.rs` (auto-wired into `Repos.chat.summarization`
//! by the chat repository's build-script walk). The summary-row SQL
//! (`fetch_summary`, `upsert_summary`) stays in
//! `engine/summarizer.rs` because it's tightly coupled to the engine's
//! `ConversationSummary` struct and decision logic.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::SummarizationAdminSettings;

#[derive(Clone, Debug)]
pub struct SummarizationRepository {
    pool: PgPool,
}

impl SummarizationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Expose a cheap clone of the pool for fire-and-forget spawns in
    /// chat extensions. Mirrors `MemoryRepository::pool_clone`.
    pub fn pool_clone(&self) -> PgPool {
        self.pool.clone()
    }

    // ── summarization_admin_settings (single row, id=1) ──────────────

    pub async fn get_admin_settings(&self) -> Result<SummarizationAdminSettings, AppError> {
        let row = sqlx::query_as!(
            SummarizationAdminSettings,
            r#"
            SELECT
                id,
                enabled,
                default_summarization_model_id,
                summarize_after_tokens,
                summarizer_keep_recent_tokens,
                full_summary_prompt,
                incremental_summary_prompt,
                updated_at as "updated_at: _"
            FROM summarization_admin_settings
            WHERE id = 1
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_admin_settings(
        &self,
        enabled: Option<bool>,
        default_summarization_model_id: Option<Option<Uuid>>,
        summarize_after_tokens: Option<i32>,
        summarizer_keep_recent_tokens: Option<i32>,
        full_summary_prompt: Option<Option<String>>,
        incremental_summary_prompt: Option<Option<String>>,
    ) -> Result<SummarizationAdminSettings, AppError> {
        // Tri-state split: outer Some ⇒ "client sent this key";
        // inner Some/None ⇒ value-vs-null. The boolean drives the
        // CASE WHEN ... ELSE column END pattern so absent fields don't
        // clobber the row.
        let model_set = default_summarization_model_id.is_some();
        let model_val = default_summarization_model_id.flatten();
        let full_prompt_set = full_summary_prompt.is_some();
        let full_prompt_val = full_summary_prompt.flatten();
        let inc_prompt_set = incremental_summary_prompt.is_some();
        let inc_prompt_val = incremental_summary_prompt.flatten();

        let row = sqlx::query_as!(
            SummarizationAdminSettings,
            r#"
            UPDATE summarization_admin_settings
            SET enabled                        = COALESCE($1, enabled),
                default_summarization_model_id = CASE WHEN $2::bool THEN $3 ELSE default_summarization_model_id END,
                summarize_after_tokens         = COALESCE($4, summarize_after_tokens),
                summarizer_keep_recent_tokens  = COALESCE($5, summarizer_keep_recent_tokens),
                full_summary_prompt            = CASE WHEN $6::bool THEN $7 ELSE full_summary_prompt END,
                incremental_summary_prompt     = CASE WHEN $8::bool THEN $9 ELSE incremental_summary_prompt END,
                updated_at                     = NOW()
            WHERE id = 1
            RETURNING
                id,
                enabled,
                default_summarization_model_id,
                summarize_after_tokens,
                summarizer_keep_recent_tokens,
                full_summary_prompt,
                incremental_summary_prompt,
                updated_at as "updated_at: _"
            "#,
            enabled,
            model_set,
            model_val,
            summarize_after_tokens,
            summarizer_keep_recent_tokens,
            full_prompt_set,
            full_prompt_val,
            inc_prompt_set,
            inc_prompt_val,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}
