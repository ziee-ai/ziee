//! Summarization chat-extension repository.
//!
//! Owns the `conversation_summarization_settings` table (migration 91)
//! — per-conversation `inherit` / `on` / `off` override that mirrors
//! `conversation_memory_settings`. Auto-wired into `ChatRepository` as
//! `Repos.chat.summarization` by the server's chat-repository build-
//! script walk over `modules/<sibling>/chat_extension/repository.rs`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

/// Vocabulary for the per-conversation toggle. Row absence means
/// `'inherit'` — the row is only created on first non-default write
/// (keeps storage tight + semantics consistent with migration 91).
pub const DEFAULT_SUMMARIZATION_MODE: &str = "inherit";

#[derive(Clone, Debug)]
pub struct SummarizationChatRepository {
    pool: PgPool,
}

impl SummarizationChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the per-conversation summarization mode. Returns
    /// `'inherit'` when no row exists. No ownership gating; intended
    /// for internal use from the chat extension (caller knows the
    /// conversation belongs to the active user via `StreamContext`).
    pub async fn get_conversation_summarization_mode(
        &self,
        conversation_id: Uuid,
    ) -> Result<String, AppError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT summarization_mode FROM conversation_summarization_settings WHERE conversation_id = $1",
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row
            .map(|(m,)| m)
            .unwrap_or_else(|| DEFAULT_SUMMARIZATION_MODE.to_string()))
    }

    /// Set the per-conversation summarization mode. Writing
    /// `'inherit'` deletes the row (row absence == inherit), saving
    /// storage and keeping semantics consistent. Non-`'inherit'`
    /// values upsert.
    ///
    /// The DB CHECK constraint validates the vocabulary; callers
    /// should still pre-validate to return a clean 400 instead of a
    /// 500 from the DB.
    pub async fn set_conversation_summarization_mode(
        &self,
        conversation_id: Uuid,
        mode: &str,
    ) -> Result<(), AppError> {
        if mode == DEFAULT_SUMMARIZATION_MODE {
            sqlx::query(
                "DELETE FROM conversation_summarization_settings WHERE conversation_id = $1",
            )
            .bind(conversation_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO conversation_summarization_settings (conversation_id, summarization_mode)
                VALUES ($1, $2)
                ON CONFLICT (conversation_id) DO UPDATE SET summarization_mode = EXCLUDED.summarization_mode,
                                                            updated_at = NOW()
                "#,
            )
            .bind(conversation_id)
            .bind(mode)
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        }
        Ok(())
    }

    /// Ownership-gated read for the HTTP endpoints. Returns:
    ///   - `Ok(None)` when the user doesn't own the conversation
    ///     (caller maps to 404 to defeat probing for conversation ids).
    ///   - `Ok(Some(mode))` when the user owns the conversation; mode
    ///     defaults to `'inherit'` when no row exists.
    pub async fn get_for_user(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let owns: Option<bool> = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM conversations
                WHERE id = $1 AND user_id = $2
            )
            "#,
            conversation_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        if !owns.unwrap_or(false) {
            return Ok(None);
        }
        Ok(Some(
            self.get_conversation_summarization_mode(conversation_id).await?,
        ))
    }
}
