use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::models::ConversationFile;

/// Repository for the code_sandbox module.
///
/// Owns three concerns:
/// 1. Lookup of conversation files (joined through the branching schema)
///    so tools can expose user attachments as read-only binds.
/// 2. File-id-scoped lookups for `get_resource_link`.
/// 3. Boot-time upsert of the built-in MCP server row.
#[derive(Clone)]
pub struct CodeSandboxRepository {
    pool: PgPool,
}

impl CodeSandboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Resolve the user who owns the given conversation.
    pub async fn get_conversation_user_id(
        &self,
        _conversation_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        // Implemented in Phase 2.
        Ok(None)
    }

    /// Files attached to the conversation's active branch. Walks
    /// `conversations.active_branch_id → branch_messages → messages →
    /// message_contents` and extracts `file_id` from the JSONB content
    /// for content_type IN ('file_attachment', 'image').
    pub async fn get_conversation_files(
        &self,
        _conversation_id: Uuid,
    ) -> Result<Vec<ConversationFile>, AppError> {
        // Implemented in Phase 2.
        Ok(Vec::new())
    }

    /// Fetch a single file by id, scoped to the user that owns it
    /// (foreign-attachment access is denied at query time).
    pub async fn get_file_by_id(
        &self,
        _file_id: Uuid,
        _user_id: Uuid,
    ) -> Result<Option<ConversationFile>, AppError> {
        // Implemented in Phase 2.
        Ok(None)
    }

    /// Idempotent upsert of the built-in sandbox MCP server row.
    /// **Critical:** the `ON CONFLICT DO UPDATE SET` clause must NOT
    /// include `enabled`, so admin disabling via the UI survives
    /// process restarts.
    pub async fn upsert_builtin_server(
        &self,
        _server_id: Uuid,
        _loopback_url: &str,
    ) -> Result<(), AppError> {
        // Implemented in Phase 6.
        Ok(())
    }
}
