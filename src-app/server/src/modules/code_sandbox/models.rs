use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// A file attached to a conversation, surfaced by `get_conversation_files`.
/// Used by sandbox tools to expose user-uploaded files as read-only binds
/// at their original filenames.
#[derive(Debug, Clone, FromRow)]
pub struct ConversationFile {
    pub file_id: Uuid,
    pub filename: String,
    pub user_id: Uuid,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
}
