use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ConversationFile {
    pub file_id: Uuid,
    pub filename: String,
    pub user_id: Uuid,
    pub mime_type: Option<String>,
    pub created_at: time::OffsetDateTime,
}
