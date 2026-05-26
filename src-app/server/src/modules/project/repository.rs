// Project repository.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::Project;
use super::types::{
    CreateProjectRequest, ProjectFileListResponse, ProjectListResponse,
    UpdateProjectMcpSettingsRequest, UpdateProjectRequest,
};
use crate::common::AppError;
use crate::modules::file::models::File as FileEntity;

/// Hard cap on project files (Tier-1 validator gate). Matches the v1
/// design in Plan 5 §8 ("File count cap — 100 files per project").
pub const PROJECT_MAX_FILES: i64 = 100;

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
        let approval_mode = req
            .mcp_approval_mode
            .as_deref()
            .unwrap_or("manual_approve")
            .to_string();
        // Serialize the strict types back to JSONB for storage. The
        // type-level validation (Vec<McpServerToolEntry>) has already
        // rejected shape errors at handler ingress.
        //
        // `serde_json::to_value` only fails for types with non-JSON-
        // serializable fields (NaN floats, non-string Map keys). Our
        // McpServerToolEntry has neither, so failure is unreachable
        // in practice — but surface it explicitly instead of silently
        // storing `[]`, which would destroy user input. If the type
        // ever evolves to add a non-serializable field, we crash loudly
        // instead of corrupting data.
        let auto_approved = match req.mcp_auto_approved_tools {
            Some(entries) => serde_json::to_value(entries).map_err(|e| {
                tracing::error!(error = %e, "failed to serialize mcp_auto_approved_tools");
                AppError::internal_error("Failed to serialize MCP settings")
            })?,
            None => serde_json::json!([]),
        };
        let disabled_servers = match req.mcp_disabled_servers {
            Some(entries) => serde_json::to_value(entries).map_err(|e| {
                tracing::error!(error = %e, "failed to serialize mcp_disabled_servers");
                AppError::internal_error("Failed to serialize MCP settings")
            })?,
            None => serde_json::json!([]),
        };

        // loop_settings: NULL at create time means "use application
        // defaults" — same convention as conversation_mcp_settings.
        // CreateProjectRequest doesn't expose loop_settings (the
        // dedicated MCP-settings PUT endpoint sets it later), but we
        // pass NULL explicitly so the bind position is unambiguous.
        let loop_settings: Option<serde_json::Value> = None;

        let project = sqlx::query_as!(
            Project,
            r#"
            INSERT INTO projects (
                user_id, name, description, instructions,
                default_assistant_id, default_model_id,
                mcp_approval_mode, mcp_auto_approved_tools, mcp_disabled_servers,
                mcp_loop_settings
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id,
            req.name,
            req.description,
            req.instructions,
            req.default_assistant_id,
            req.default_model_id,
            approval_mode,
            auto_approved,
            disabled_servers,
            loop_settings,
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
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
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
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
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
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
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

    pub async fn update_mcp_settings(
        &self,
        id: Uuid,
        user_id: Uuid,
        req: UpdateProjectMcpSettingsRequest,
    ) -> Result<Project, AppError> {
        // Same rationale as `create`: surface serialization failures
        // explicitly rather than silently truncating to `[]`.
        let auto_approved = serde_json::to_value(&req.auto_approved_tools).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize mcp auto_approved_tools");
            AppError::internal_error("Failed to serialize MCP settings")
        })?;
        let disabled_servers = serde_json::to_value(&req.disabled_servers).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize mcp disabled_servers");
            AppError::internal_error("Failed to serialize MCP settings")
        })?;

        // loop_settings: pass Option<Value> straight through. NULL is a
        // first-class value here ("not configured — use defaults"); a
        // None on the request preserves any existing row value via the
        // explicit binding below (we still set it — `None` writes NULL,
        // which is the documented "use defaults" semantic).
        let loop_settings = req.loop_settings.clone();

        let project = sqlx::query_as!(
            Project,
            r#"
            UPDATE projects
            SET mcp_approval_mode = $3,
                mcp_auto_approved_tools = $4,
                mcp_disabled_servers = $5,
                mcp_loop_settings = $6,
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            id,
            user_id,
            req.approval_mode,
            auto_approved,
            disabled_servers,
            loop_settings,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        project.ok_or_else(|| AppError::not_found("Project"))
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

    // ============ Files ============

    /// Attach a file. Idempotent: duplicate (composite PK collision) is
    /// silently swallowed and returns success. The caller must have
    /// already verified that `file_id`'s owner matches the project's
    /// owner — repository does NOT re-check; handler is the boundary.
    ///
    /// For RACE-FREE attach that also enforces the 100-file cap, use
    /// `attach_file_capped` instead — that variant takes a row lock on
    /// the project and recounts before INSERT to close audit B1.
    pub async fn attach_file(&self, project_id: Uuid, file_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO project_files (project_id, file_id)
            VALUES ($1, $2)
            ON CONFLICT (project_id, file_id) DO NOTHING
            "#,
            project_id,
            file_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Race-free attach that enforces the file count cap atomically.
    /// Closes audit B1: two concurrent attaches at count=99 used to
    /// both pass a pre-check and result in count=101. Now we take a
    /// `FOR UPDATE` row lock on the project, count under the lock,
    /// reject if at cap, and insert in the same transaction. Returns
    /// `Ok(true)` if a new row was inserted, `Ok(false)` if the file
    /// was already attached (idempotent path — cap not consulted).
    pub async fn attach_file_capped(
        &self,
        project_id: Uuid,
        file_id: Uuid,
        cap: i64,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Lock the project row so no concurrent attach can race past
        // the count check. Cheap: one row per project.
        let project_locked = sqlx::query_scalar!(
            "SELECT 1 FROM projects WHERE id = $1 FOR UPDATE",
            project_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        if project_locked.is_none() {
            return Err(AppError::not_found("Project"));
        }

        // Already attached? Idempotent — don't count toward cap.
        let already: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1 AND file_id = $2",
            project_id,
            file_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if already > 0 {
            tx.commit().await.map_err(AppError::database_error)?;
            return Ok(false);
        }

        // Recount under the lock — this is the load-bearing step.
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1",
            project_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if count >= cap {
            // Roll back the lock + return a sentinel error the handler
            // converts to a 422.
            return Err(AppError::unprocessable_entity(
                "PROJECT_FILE_COUNT_CAP",
                format!("Project file count cap ({cap}) reached"),
            ));
        }

        sqlx::query!(
            r#"
            INSERT INTO project_files (project_id, file_id)
            VALUES ($1, $2)
            ON CONFLICT (project_id, file_id) DO NOTHING
            "#,
            project_id,
            file_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(true)
    }

    pub async fn detach_file(&self, project_id: Uuid, file_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM project_files WHERE project_id = $1 AND file_id = $2",
            project_id,
            file_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_files(&self, project_id: Uuid) -> Result<i64, AppError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1",
            project_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        Ok(count)
    }

    /// List file IDs only — fast path for the chat/project extension
    /// which converts them into provider-specific ContentBlocks.
    pub async fn list_file_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query!(
            "SELECT file_id FROM project_files WHERE project_id = $1 ORDER BY added_at ASC",
            project_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| r.file_id).collect())
    }

    /// List files with metadata (JOIN on files). Returns the same File
    /// entity the file module returns, for client convenience.
    pub async fn list_files(
        &self,
        project_id: Uuid,
    ) -> Result<ProjectFileListResponse, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                f.id, f.user_id, f.filename, f.file_size,
                f.mime_type, f.checksum, f.has_thumbnail,
                f.preview_page_count, f.text_page_count,
                f.processing_metadata, f.created_by,
                f.created_at, f.updated_at
            FROM project_files pf
            JOIN files f ON f.id = pf.file_id
            WHERE pf.project_id = $1
            ORDER BY pf.added_at ASC
            "#,
            project_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let total = rows.len() as i64;
        let files: Vec<FileEntity> = rows
            .into_iter()
            .map(|r| FileEntity {
                id: r.id,
                user_id: r.user_id,
                filename: r.filename,
                file_size: r.file_size,
                mime_type: r.mime_type,
                checksum: r.checksum,
                has_thumbnail: r.has_thumbnail,
                preview_page_count: r.preview_page_count,
                text_page_count: r.text_page_count,
                processing_metadata: r.processing_metadata.unwrap_or_else(|| serde_json::json!({})),
                created_by: r.created_by,
                created_at: chrono::DateTime::from_timestamp(r.created_at.unix_timestamp(), 0)
                    .unwrap(),
                updated_at: chrono::DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0)
                    .unwrap(),
            })
            .collect();

        Ok(ProjectFileListResponse { files, total })
    }

    // ============ Conversations ============

    /// Resolve a project from a conversation (used by the chat/project
    /// extension to inject context). Returns None when the conversation
    /// has no project OR belongs to a different user (safety: never
    /// inject a foreign user's instructions/files).
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
                p.mcp_approval_mode,
                p.mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                p.mcp_disabled_servers as "mcp_disabled_servers!: _",
                p.mcp_loop_settings,
                p.created_at as "created_at: _",
                p.updated_at as "updated_at: _"
            FROM conversations c
            JOIN projects p ON p.id = c.project_id
            WHERE c.id = $1 AND c.user_id = $2 AND p.user_id = $2
            "#,
            conversation_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(project)
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
    pub async fn duplicate(&self, id: Uuid, user_id: Uuid) -> Result<Project, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        let original = sqlx::query_as!(
            Project,
            r#"
            SELECT
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            FROM projects
            WHERE id = $1 AND user_id = $2
            FOR UPDATE
            "#,
            id,
            user_id
        )
        .fetch_optional(&mut *tx)
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
            .fetch_one(&mut *tx)
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
            .fetch_one(&mut *tx)
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
                default_assistant_id, default_model_id,
                mcp_approval_mode, mcp_auto_approved_tools, mcp_disabled_servers,
                mcp_loop_settings
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id, user_id, name, description, instructions,
                default_assistant_id as "default_assistant_id: _",
                default_model_id as "default_model_id: _",
                mcp_approval_mode,
                mcp_auto_approved_tools as "mcp_auto_approved_tools!: _",
                mcp_disabled_servers as "mcp_disabled_servers!: _",
                mcp_loop_settings,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
            "#,
            user_id,
            candidate,
            original.description,
            original.instructions,
            original.default_assistant_id,
            original.default_model_id,
            original.mcp_approval_mode,
            original.mcp_auto_approved_tools,
            original.mcp_disabled_servers,
            original.mcp_loop_settings,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        // Copy project_files membership (references the SAME file rows,
        // not copies — see Plan 5 §8 "Duplicate-project file references
        // are shared, not copied").
        sqlx::query!(
            r#"
            INSERT INTO project_files (project_id, file_id, added_at)
            SELECT $1, file_id, NOW()
            FROM project_files
            WHERE project_id = $2
            "#,
            new_project.id,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(new_project)
    }

    // ============ MCP snapshot helper ============

    /// Snapshot this project's MCP settings into the
    /// conversation_mcp_settings table for a newly-created conversation.
    /// Idempotent — uses ON CONFLICT DO NOTHING so re-snapshotting an
    /// already-configured conversation is a no-op (the per-conversation
    /// override wins).
    pub async fn snapshot_mcp_into_conversation(
        &self,
        executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
        project_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
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
            WHERE p.id = $3
            ON CONFLICT (conversation_id) DO NOTHING
            "#,
            conversation_id,
            user_id,
            project_id,
        )
        .execute(executor)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }
}
