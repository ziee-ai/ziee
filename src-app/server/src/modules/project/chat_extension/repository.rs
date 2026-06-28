// Repository methods for the project↔chat bridge.
//
// Owns ONLY the JOIN-and-return-ConversationResponse query — the one
// piece of project-conversation logic that legitimately imports chat
// types. The pure project_conversations CRUD (`attach_*`,
// `detach_*`, `get_for_conversation`, `user_owns_conversation`,
// `project_id_for_conversation`) stays in `project/repository.rs` since
// it operates on `Uuid`s without touching chat types.
//
// Auto-wired into `ChatRepository` as `Repos.chat.project` by the
// server's `generate_chat_repository` build-script walk over
// `modules/<sibling>/chat_extension/repository.rs` (build.rs:288+).

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Conversation;
use crate::modules::chat::core::types::ConversationResponse;

#[derive(Clone, Debug)]
pub struct ProjectChatRepository {
    pool: PgPool,
}

impl ProjectChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List conversations attached to a project, with paging. The
    /// caller (project handler) must have already verified
    /// `user_id` owns `project_id`; the `c.user_id = $2` clause is
    /// defense-in-depth.
    pub async fn list_conversations_in_project(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ConversationResponse>, AppError> {
        // Page the conversations FIRST, then count messages only for that page
        // — the message_count subquery touches branches/branch_messages for the
        // ≤LIMIT rows on this page instead of joining + grouping every
        // conversation in the project before applying the LIMIT.
        let rows = sqlx::query!(
            r#"
            WITH page AS (
                SELECT c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
                       c.created_at, c.updated_at
                FROM project_conversations pc
                JOIN conversations c ON c.id = pc.conversation_id
                WHERE pc.project_id = $1 AND c.user_id = $2
                ORDER BY c.updated_at DESC
                LIMIT $3 OFFSET $4
            )
            SELECT
                p.id, p.user_id, p.model_id, p.title, p.active_branch_id,
                p.created_at, p.updated_at,
                (SELECT COUNT(bm.message_id)
                   FROM branches b
                   JOIN branch_messages bm ON bm.branch_id = b.id
                  WHERE b.conversation_id = p.id) AS message_count
            FROM page p
            ORDER BY p.updated_at DESC
            "#,
            project_id,
            user_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let to_chrono = |odt: time::OffsetDateTime| -> chrono::DateTime<chrono::Utc> {
            chrono::DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond())
                .expect("valid timestamp")
        };

        Ok(rows
            .into_iter()
            .map(|row| ConversationResponse {
                conversation: Conversation {
                    id: row.id,
                    user_id: row.user_id,
                    model_id: row.model_id,
                    title: row.title,
                    active_branch_id: row.active_branch_id,
                    created_at: to_chrono(row.created_at),
                    updated_at: to_chrono(row.updated_at),
                },
                message_count: row.message_count.unwrap_or(0),
            })
            .collect())
    }
}
