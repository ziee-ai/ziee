//! Assistant chat-extension repository.
//!
//! Owns the `message_assistant` join table (migration 75) that
//! replaced chat's `messages.assistant_id` column. Used by:
//!   - `after_user_message_created` hook (insert) — records which
//!     assistant was selected when a user message was sent.
//!   - `GET /api/messages/{id}/assistant` endpoint (get) — frontend
//!     assistant extension hits this on message Edit to restore the
//!     original assistant selection.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

/// Repository for the assistant chat-extension's owned tables.
/// Auto-wired into `ChatRepository` as `Repos.chat.assistant` by the
/// server's `generate_chat_repository` build-script walk over
/// `modules/<sibling>/chat_extension/repository.rs`.
#[derive(Clone, Debug)]
pub struct AssistantChatRepository {
    pool: PgPool,
}

impl AssistantChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Snapshot the assistant that was active when this user message
    /// was sent. PRIMARY KEY on `message_id` makes it idempotent —
    /// re-inserts for the same message_id no-op via ON CONFLICT.
    pub async fn insert_message_assistant(
        &self,
        message_id: Uuid,
        assistant_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO message_assistant (message_id, assistant_id)
            VALUES ($1, $2)
            ON CONFLICT (message_id) DO NOTHING
            "#,
        )
        .bind(message_id)
        .bind(assistant_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Get the assistant_id that was attributed to this message, gated
    /// by user ownership via the chat conversation chain. Returns:
    ///   - `Ok(None)` when the user doesn't own the conversation
    ///     containing the message (caller maps to 404).
    ///   - `Ok(Some(None))` when the user owns the message but no
    ///     assistant was attributed (sent without an assistant).
    ///   - `Ok(Some(Some(id)))` when the user owns the message and an
    ///     assistant was recorded.
    pub async fn get_message_assistant_for_user(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, AppError> {
        // Ownership check via the chat conversation chain.
        // Assistant's bridge is allowed to read chat's tables
        // (sibling-module → chat direction is the allowed one).
        let owns: Option<bool> = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM messages m
                INNER JOIN branch_messages bm ON bm.message_id = m.id
                INNER JOIN branches b ON b.id = bm.branch_id
                INNER JOIN conversations c ON c.id = b.conversation_id
                WHERE m.id = $1 AND c.user_id = $2
            )
            "#,
            message_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        if !owns.unwrap_or(false) {
            return Ok(None);
        }

        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT assistant_id FROM message_assistant WHERE message_id = $1",
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(Some(row.map(|(id,)| id)))
    }
}
