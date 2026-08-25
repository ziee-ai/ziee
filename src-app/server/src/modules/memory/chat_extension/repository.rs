//! Memory chat-extension repository.
//!
//! Owns the `conversation_memory_settings` table (migration 76) that
//! replaced chat's `conversations.memory_mode` column. Used by:
//!   - the bridge `retriever.rs` (read) — applies the per-conversation
//!     override to gate retrieval.
//!   - `GET /api/conversations/{id}/memory-mode` (read, user-gated) —
//!     frontend memory extension reads the current mode for the
//!     conversation settings UI.
//!   - `PUT /api/conversations/{id}/memory-mode` (write, user-gated) —
//!     frontend memory extension updates the per-conversation mode.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

/// Memory's stable vocabulary for the per-conversation toggle. The
/// implicit default (no row in `conversation_memory_settings`) is
/// `'inherit'` — the row is only created on first non-default write.
pub const DEFAULT_MEMORY_MODE: &str = "inherit";

/// Repository for the memory chat-extension's owned tables.
/// Auto-wired into `ChatRepository` as `Repos.chat.memory` by the
/// server's `generate_chat_repository` build-script walk over
/// `modules/<sibling>/chat_extension/repository.rs`.
#[derive(Clone, Debug)]
pub struct MemoryChatRepository {
    pool: PgPool,
}

impl MemoryChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the per-conversation memory mode. Returns `'inherit'`
    /// when no row exists (the implicit default — see migration 76).
    /// No ownership gating; intended for internal bridge use
    /// (`retriever.rs` already gates by user via `context.user_id`).
    pub async fn get_conversation_memory_mode(
        &self,
        conversation_id: Uuid,
    ) -> Result<String, AppError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT memory_mode FROM conversation_memory_settings WHERE conversation_id = $1",
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|(m,)| m).unwrap_or_else(|| DEFAULT_MEMORY_MODE.to_string()))
    }

    /// Set the per-conversation memory mode. Writing `'inherit'`
    /// deletes the row (row absence == 'inherit', saves storage and
    /// keeps semantics consistent with the migration's backfill).
    /// Non-`'inherit'` values upsert.
    ///
    /// The DB `CHECK` constraint validates the vocabulary; callers
    /// should still pre-validate to return a clean 400 instead of a
    /// 500 from the DB.
    pub async fn set_conversation_memory_mode(
        &self,
        conversation_id: Uuid,
        mode: &str,
    ) -> Result<(), AppError> {
        if mode == DEFAULT_MEMORY_MODE {
            sqlx::query(
                "DELETE FROM conversation_memory_settings WHERE conversation_id = $1",
            )
            .bind(conversation_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO conversation_memory_settings (conversation_id, memory_mode)
                VALUES ($1, $2)
                ON CONFLICT (conversation_id) DO UPDATE SET memory_mode = EXCLUDED.memory_mode
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
        Ok(Some(self.get_conversation_memory_mode(conversation_id).await?))
    }
}
