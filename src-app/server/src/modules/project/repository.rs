// Project repository.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::Project;
use super::types::{CreateProjectRequest, ProjectListResponse, UpdateProjectRequest};
use crate::common::AppError;

pub struct ProjectRepository {
    pool: PgPool,
}

impl ProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ============ CRUD ============

    pub async fn create(
        &self,
        user_id: Uuid,
        req: CreateProjectRequest,
    ) -> Result<Project, AppError> {
        // MCP defaults are no longer set at project-create time —
        // CreateProjectRequest drops its mcp_* fields entirely (migration 78
        // + project↔mcp inversion). Clients customize MCP via a separate
        // PUT /api/projects/{id}/mcp-settings call after create.
        let project = sqlx::query_as!(
            Project,
            r#"
            INSERT INTO projects (
                user_id, name, description, instructions,
                default_assistant_id, default_model_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id,
            req.name,
            req.description,
            req.instructions,
            req.default_assistant_id,
            req.default_model_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(project)
    }

    /// Read a project scoped to `user_id`. Returns None for projects
    /// owned by other users (404, not 403, to avoid existence leak).
    pub async fn get_for_user(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Project>, AppError> {
        let project = sqlx::query_as!(
            Project,
            r#"
            SELECT
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM projects
            WHERE id = $1 AND user_id = $2
            "#,
            id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(project)
    }

    pub async fn list_for_user(
        &self,
        user_id: Uuid,
        page: i64,
        limit: i64,
    ) -> Result<ProjectListResponse, AppError> {
        // saturating_mul guards against pathological inputs (the
        // handler clamps already prevent this, but defense-in-depth).
        let offset = (page - 1).saturating_mul(limit);

        let projects = sqlx::query_as!(
            Project,
            r#"
            SELECT
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM projects
            WHERE user_id = $1
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let total: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM projects WHERE user_id = $1",
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);

        Ok(ProjectListResponse { projects, total })
    }

    pub async fn update(
        &self,
        id: Uuid,
        user_id: Uuid,
        req: UpdateProjectRequest,
    ) -> Result<Project, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Confirm ownership first so per-field updates can rely on it.
        let exists: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM projects WHERE id = $1 AND user_id = $2",
            id,
            user_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);

        if exists == 0 {
            return Err(AppError::not_found("Project"));
        }

        // Per-field updates (matches assistant repository pattern).
        if let Some(name) = &req.name {
            sqlx::query!(
                "UPDATE projects SET name = $1, updated_at = NOW() WHERE id = $2",
                name,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
        if let Some(description) = &req.description {
            sqlx::query!(
                "UPDATE projects SET description = $1, updated_at = NOW() WHERE id = $2",
                description,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
        if let Some(instructions) = &req.instructions {
            sqlx::query!(
                "UPDATE projects SET instructions = $1, updated_at = NOW() WHERE id = $2",
                instructions,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
        if let Some(default_assistant_id) = req.default_assistant_id {
            sqlx::query!(
                "UPDATE projects SET default_assistant_id = $1, updated_at = NOW() WHERE id = $2",
                default_assistant_id,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
        if let Some(default_model_id) = req.default_model_id {
            sqlx::query!(
                "UPDATE projects SET default_model_id = $1, updated_at = NOW() WHERE id = $2",
                default_model_id,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        let project = sqlx::query_as!(
            Project,
            r#"
            SELECT
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM projects WHERE id = $1
            "#,
            id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(project)
    }

    pub async fn delete(&self, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM projects WHERE id = $1 AND user_id = $2",
            id,
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(result.rows_affected() > 0)
    }

    // ============ Conversations (project_conversations join table) ============

    /// Resolve a project from a conversation (used by the chat/project
    /// extension to inject context). Returns None when the conversation
    /// has no project membership OR the project belongs to a different
    /// user (safety: never inject a foreign user's instructions/files).
    pub async fn get_for_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Project>, AppError> {
        let project = sqlx::query_as!(
            Project,
            r#"
            SELECT
                p.id, p.user_id, p.name, p.description, p.instructions,
                p.default_assistant_id as "default_assistant_id: _",
                p.default_model_id as "default_model_id: _",
                p.created_at as "created_at: _",
                p.updated_at as "updated_at: _"
            FROM project_conversations pc
            JOIN projects p ON p.id = pc.project_id
            WHERE pc.conversation_id = $1 AND p.user_id = $2
            "#,
            conversation_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(project)
    }

    /// Return the project ID that a conversation is currently attached
    /// to (None if unfiled). Lightweight query for handlers/extensions
    /// that only need the ID, not the full project row.
    pub async fn project_id_for_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            "SELECT project_id FROM project_conversations WHERE conversation_id = $1",
            conversation_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.project_id))
    }

    /// Insert / update a conversation's project membership in the
    /// caller's transaction. PK on `conversation_id` means a
    /// conversation can be in at most one project; `ON CONFLICT`
    /// flips the project_id on a cross-project move. Returns the
    /// previous project_id (None if the conversation was unfiled),
    /// useful for event payloads.
    pub async fn attach_conversation_in_tx<'a>(
        &self,
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        project_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let prev = sqlx::query!(
            "SELECT project_id FROM project_conversations WHERE conversation_id = $1",
            conversation_id,
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::database_error)?
        .map(|r| r.project_id);

        sqlx::query!(
            r#"
            INSERT INTO project_conversations (conversation_id, project_id)
            VALUES ($1, $2)
            ON CONFLICT (conversation_id) DO UPDATE
            SET project_id = EXCLUDED.project_id,
                attached_at = NOW()
            "#,
            conversation_id,
            project_id,
        )
        .execute(&mut **tx)
        .await
        .map_err(AppError::database_error)?;

        Ok(prev)
    }

    /// Remove a conversation's project membership row in the caller's
    /// transaction. Returns true if a row was deleted (the
    /// conversation was actually in that project), false otherwise.
    pub async fn detach_conversation_in_tx<'a>(
        &self,
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        project_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM project_conversations WHERE conversation_id = $1 AND project_id = $2",
            conversation_id,
            project_id,
        )
        .execute(&mut **tx)
        .await
        .map_err(AppError::database_error)?;
        Ok(result.rows_affected() > 0)
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
    ) -> Result<Vec<crate::modules::chat::core::types::ConversationResponse>, AppError> {
        use crate::modules::chat::core::models::Conversation;
        use crate::modules::chat::core::types::ConversationResponse;
        let rows = sqlx::query!(
            r#"
            SELECT
                c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
                c.created_at, c.updated_at,
                COUNT(bm.message_id) as message_count
            FROM project_conversations pc
            JOIN conversations c ON c.id = pc.conversation_id
            LEFT JOIN branches b ON b.conversation_id = c.id
            LEFT JOIN branch_messages bm ON bm.branch_id = b.id
            WHERE pc.project_id = $1 AND c.user_id = $2
            GROUP BY c.id
            ORDER BY c.updated_at DESC
            LIMIT $3 OFFSET $4
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

    /// Verify a conversation is owned by `user_id`. Used by attach/
    /// detach handlers to prevent cross-user mutation before touching
    /// the join table.
    pub async fn user_owns_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, AppError> {
        let row = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1 AND user_id = $2)",
            conversation_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.unwrap_or(false))
    }

    // ============ Duplicate ============

    /// Clone a project: copies all scalar columns + project_files rows
    /// (referring to the same files). Does NOT copy conversations or
    /// messages. Name disambiguation appends " (copy)", " (copy 2)", …
    /// until the per-user unique constraint is satisfied.
    ///
    /// Takes a `FOR UPDATE` lock on the original project row so two
    /// concurrent duplicates of the same source serialize cleanly
    /// (audit N3). Without the lock, both could compute the same
    /// "(copy N)" suffix as free and one would fail with a unique-
    /// constraint 500 from the INSERT.
    /// Duplicate a project row (instructions + defaults + MCP settings).
    ///
    /// File-attachment cloning is NOT done here — that's the file
    /// module's responsibility via its `ProjectExtension::on_project_duplicated`
    /// hook. The handler opens the outer transaction, calls this method,
    /// fans out to all project extensions, then commits — so the project
    /// row and all per-extension state share atomicity.
    pub async fn duplicate_in_tx<'a>(
        &self,
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Project, AppError> {
        let original = sqlx::query_as!(
            Project,
            r#"
            SELECT
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM projects
            WHERE id = $1 AND user_id = $2
            FOR UPDATE
            "#,
            id,
            user_id
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Project"))?;

        // Find an unused " (copy [N])" suffix. Cap at 999 attempts to
        // avoid pathological behavior if a user somehow accumulates
        // hundreds of "Foo (copy N)" rows. If we exhaust the loop
        // without finding a free name, surface a 422 with a clear
        // error code rather than letting the subsequent INSERT fail
        // with an opaque unique-constraint 500 (closes audit B6).
        let base_name = original.name.clone();
        let mut candidate = format!("{} (copy)", base_name);
        let mut found_free = false;
        for n in 2..1000 {
            let collision: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM projects WHERE user_id = $1 AND name = $2",
                user_id,
                candidate
            )
            .fetch_one(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .unwrap_or(0);
            if collision == 0 {
                found_free = true;
                break;
            }
            candidate = format!("{} (copy {})", base_name, n);
        }
        // After the loop, candidate is set to "Foo (copy 999)" if all
        // 998 previous suffixes were taken. Check the LAST candidate
        // explicitly (the loop only checks 2..999, so 999 is checked
        // here on exit).
        if !found_free {
            let collision: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM projects WHERE user_id = $1 AND name = $2",
                user_id,
                candidate
            )
            .fetch_one(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .unwrap_or(0);
            if collision > 0 {
                return Err(AppError::unprocessable_entity(
                    "PROJECT_DUPLICATE_LIMIT",
                    "Cannot duplicate: too many copies already exist (limit 999). \
                     Delete some \"(copy N)\" projects and try again.",
                ));
            }
        }

        let new_project = sqlx::query_as!(
            Project,
            r#"
            INSERT INTO projects (
                user_id, name, description, instructions,
                default_assistant_id, default_model_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id,
            candidate,
            original.description,
            original.instructions,
            original.default_assistant_id,
            original.default_model_id,
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(AppError::database_error)?;

        // project_files clone runs via FileProjectExtension::on_project_duplicated
        // (project↔file inversion). MCP settings clone runs via
        // McpProjectExtension::on_project_duplicated (project↔mcp inversion).
        // Caller commits the outer transaction after firing the extension
        // fan-out.
        Ok(new_project)
    }

}
