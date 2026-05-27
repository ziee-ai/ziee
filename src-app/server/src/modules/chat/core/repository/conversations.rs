// Conversations repository

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Conversation;
use crate::modules::chat::core::types::ConversationResponse;

/// Convert time::OffsetDateTime to chrono::DateTime<Utc>
fn to_chrono_datetime(odt: OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond()).expect("valid timestamp")
}

/// Create a new conversation with a default branch.
///
/// If `project_id` is set:
///   * the project must be owned by the same user (returns 404 otherwise)
///   * if `model_id` is None, snapshots the project's `default_model_id`
///     into the conversation
///   * snapshots the project's MCP defaults into conversation_mcp_settings
///     (idempotent via ON CONFLICT DO NOTHING)
pub async fn create_conversation(
    pool: &PgPool,
    user_id: Uuid,
    model_id: Option<Uuid>,
    title: Option<String>,
    project_id: Option<Uuid>,
) -> Result<Conversation, AppError> {
    // Validate model_id exists if provided
    if let Some(mid) = model_id {
        let model_exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM llm_models WHERE id = $1)",
            mid
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(false);

        if !model_exists {
            return Err(AppError::not_found("Model"));
        }
    }

    // Validate project_id exists + belongs to the same user, and resolve
    // model snapshot if needed.
    let mut effective_model_id = model_id;
    if let Some(pid) = project_id {
        let proj_row = sqlx::query!(
            "SELECT default_model_id FROM projects WHERE id = $1 AND user_id = $2",
            pid,
            user_id
        )
        .fetch_optional(pool)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Project"))?;

        // Snapshot default_model_id if no explicit model was passed in.
        if effective_model_id.is_none() {
            effective_model_id = proj_row.default_model_id;
        }
    }

    // Start transaction to ensure conversation and default branch are created atomically
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // 1. Create conversation (without active_branch_id yet)
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        INSERT INTO conversations (user_id, model_id, title, project_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  memory_mode,
                  project_id as "project_id: _",
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        user_id,
        effective_model_id as Option<Uuid>,
        title,
        project_id as Option<Uuid>,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 2. Create default branch
    let branch = sqlx::query!(
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, NULL, NULL)
        RETURNING id
        "#,
        conversation.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 3. Update conversation with active_branch_id
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        UPDATE conversations
        SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  memory_mode,
                  project_id as "project_id: _",
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        branch.id,
        conversation.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 4. Snapshot project MCP settings if this conversation belongs to
    // a project. We inline the snapshot SQL here (rather than calling
    // Repos.project.snapshot_mcp_into_conversation) so the whole create
    // runs in a single transaction with the rest.
    if let Some(pid) = project_id {
        sqlx::query!(
            r#"
            INSERT INTO conversation_mcp_settings (
                conversation_id, user_id,
                approval_mode, auto_approved_tools, disabled_servers, loop_settings
            )
            SELECT $1, $2,
                   p.mcp_approval_mode, p.mcp_auto_approved_tools, p.mcp_disabled_servers,
                   p.mcp_loop_settings
            FROM projects p
            WHERE p.id = $3 AND p.user_id = $2
            ON CONFLICT (conversation_id) DO NOTHING
            "#,
            conversation.id,
            user_id,
            pid,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    // Commit transaction
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Get conversation by ID (with user ownership check)
pub async fn get_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<Conversation>, AppError> {
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
               memory_mode,
               project_id as "project_id: _",
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Filter conversations by project membership in the list endpoint.
///
/// `Any` is the unchanged "all conversations" behavior. `Unfiled`
/// selects only conversations with `project_id IS NULL` (the "Recent —
/// unfiled" widget). `InProject(id)` scopes to one project (used by
/// the project handler indirectly via `list_conversations_by_project`).
#[derive(Debug, Clone, Copy)]
pub enum ConversationProjectFilter {
    Any,
    Unfiled,
    InProject(Uuid),
}

/// List conversations for a user with pagination
pub async fn list_conversations(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationResponse>, AppError> {
    list_conversations_filtered(pool, user_id, ConversationProjectFilter::Any, limit, offset).await
}

/// List conversations for a user, optionally filtered by project
/// membership. Used by the unfiled "Recent" widget and the project
/// detail page.
///
/// Each match arm materializes its own `Vec<ConversationResponse>` so
/// we don't have to unify the anonymous Record types that `sqlx::query!`
/// generates per call site.
pub async fn list_conversations_filtered(
    pool: &PgPool,
    user_id: Uuid,
    filter: ConversationProjectFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationResponse>, AppError> {
    match filter {
        ConversationProjectFilter::Any => {
            let rows = sqlx::query!(
                r#"
                SELECT
                    c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
                    c.memory_mode, c.project_id, c.created_at, c.updated_at,
                    COUNT(bm.message_id) as message_count
                FROM conversations c
                LEFT JOIN branches b ON b.conversation_id = c.id
                LEFT JOIN branch_messages bm ON bm.branch_id = b.id
                WHERE c.user_id = $1
                GROUP BY c.id
                ORDER BY c.updated_at DESC
                LIMIT $2 OFFSET $3
                "#,
                user_id,
                limit,
                offset
            )
            .fetch_all(pool)
            .await
            .map_err(AppError::database_error)?;

            Ok(rows
                .into_iter()
                .map(|row| ConversationResponse {
                    conversation: Conversation {
                        id: row.id,
                        user_id: row.user_id,
                        model_id: row.model_id,
                        title: row.title,
                        active_branch_id: row.active_branch_id,
                        memory_mode: row.memory_mode,
                        project_id: row.project_id,
                        created_at: to_chrono_datetime(row.created_at),
                        updated_at: to_chrono_datetime(row.updated_at),
                    },
                    message_count: row.message_count.unwrap_or(0),
                })
                .collect())
        }
        ConversationProjectFilter::Unfiled => {
            let rows = sqlx::query!(
                r#"
                SELECT
                    c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
                    c.memory_mode, c.project_id, c.created_at, c.updated_at,
                    COUNT(bm.message_id) as message_count
                FROM conversations c
                LEFT JOIN branches b ON b.conversation_id = c.id
                LEFT JOIN branch_messages bm ON bm.branch_id = b.id
                WHERE c.user_id = $1 AND c.project_id IS NULL
                GROUP BY c.id
                ORDER BY c.updated_at DESC
                LIMIT $2 OFFSET $3
                "#,
                user_id,
                limit,
                offset
            )
            .fetch_all(pool)
            .await
            .map_err(AppError::database_error)?;

            Ok(rows
                .into_iter()
                .map(|row| ConversationResponse {
                    conversation: Conversation {
                        id: row.id,
                        user_id: row.user_id,
                        model_id: row.model_id,
                        title: row.title,
                        active_branch_id: row.active_branch_id,
                        memory_mode: row.memory_mode,
                        project_id: row.project_id,
                        created_at: to_chrono_datetime(row.created_at),
                        updated_at: to_chrono_datetime(row.updated_at),
                    },
                    message_count: row.message_count.unwrap_or(0),
                })
                .collect())
        }
        ConversationProjectFilter::InProject(project_id) => {
            let rows = sqlx::query!(
                r#"
                SELECT
                    c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
                    c.memory_mode, c.project_id, c.created_at, c.updated_at,
                    COUNT(bm.message_id) as message_count
                FROM conversations c
                LEFT JOIN branches b ON b.conversation_id = c.id
                LEFT JOIN branch_messages bm ON bm.branch_id = b.id
                WHERE c.user_id = $1 AND c.project_id = $2
                GROUP BY c.id
                ORDER BY c.updated_at DESC
                LIMIT $3 OFFSET $4
                "#,
                user_id,
                project_id,
                limit,
                offset
            )
            .fetch_all(pool)
            .await
            .map_err(AppError::database_error)?;

            Ok(rows
                .into_iter()
                .map(|row| ConversationResponse {
                    conversation: Conversation {
                        id: row.id,
                        user_id: row.user_id,
                        model_id: row.model_id,
                        title: row.title,
                        active_branch_id: row.active_branch_id,
                        memory_mode: row.memory_mode,
                        project_id: row.project_id,
                        created_at: to_chrono_datetime(row.created_at),
                        updated_at: to_chrono_datetime(row.updated_at),
                    },
                    message_count: row.message_count.unwrap_or(0),
                })
                .collect())
        }
    }
}

/// Update conversation metadata.
///
/// All fields use the existing tri-state (None = no change, Some(None)
/// = clear, Some(Some(v)) = set). For `project_id`, the destination
/// project's ownership is verified — moving to a project not owned by
/// `user_id` returns a 403.
pub async fn update_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    title: Option<Option<String>>,
    project_id: Option<Option<Uuid>>,
) -> Result<Option<Conversation>, AppError> {
    // Confirm the conversation exists + is owned (returns 404 otherwise).
    let _existing = sqlx::query!(
        "SELECT id FROM conversations WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    if _existing.is_none() {
        return Ok(None);
    }

    // For project assignments, verify the destination project is owned
    // by the same user.
    if let Some(Some(dest_pid)) = project_id {
        let proj_count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM projects WHERE id = $1 AND user_id = $2",
            dest_pid,
            user_id
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if proj_count == 0 {
            return Err(AppError::forbidden(
                "PROJECT_ACCESS_DENIED",
                "Destination project not owned by user",
            ));
        }
    }

    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    if let Some(new_title) = title {
        sqlx::query!(
            "UPDATE conversations SET title = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3",
            new_title as Option<String>,
            id,
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(new_project_id) = project_id {
        sqlx::query!(
            "UPDATE conversations SET project_id = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3",
            new_project_id as Option<Uuid>,
            id,
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
               memory_mode,
               project_id as "project_id: _",
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(conversation)
}

/// Delete conversation (cascades to branches and messages)
pub async fn delete_conversation(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected() > 0)
}

/// Update conversation model and optionally active branch
pub async fn update_conversation_state(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
    model_id: Uuid,
    branch_id: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(branch_id) = branch_id {
        // Update both model and branch
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, active_branch_id = $2, updated_at = NOW()
            WHERE id = $3 AND user_id = $4
            "#,
            model_id,
            branch_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    } else {
        // Update only model
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, updated_at = NOW()
            WHERE id = $2 AND user_id = $3
            "#,
            model_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }

    Ok(())
}
