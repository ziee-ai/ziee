use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::ConversationFile;

pub struct CodeSandboxRepository {
    pool: PgPool,
}

impl CodeSandboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get all files referenced in the active branch of a conversation.
    /// Joins: conversations → active_branch → branch_messages → messages → message_contents → files
    pub async fn get_conversation_files(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<ConversationFile>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT
                f.id as file_id,
                f.filename,
                f.user_id,
                f.mime_type,
                f.created_at
            FROM conversations c
            INNER JOIN branches b ON b.id = c.active_branch_id
            INNER JOIN branch_messages bm ON bm.branch_id = b.id
            INNER JOIN messages m ON m.id = bm.message_id
            INNER JOIN message_contents mc ON mc.message_id = m.id
            INNER JOIN files f ON (mc.content->>'file_id')::uuid = f.id
            WHERE c.id = $1
              AND c.user_id = $2
              AND mc.content_type IN ('file_attachment', 'image')
            "#,
            conversation_id,
            user_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(rows
            .into_iter()
            .map(|r| ConversationFile {
                file_id: r.file_id,
                filename: r.filename,
                user_id: r.user_id,
                mime_type: r.mime_type,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Get a single file by its ID, scoped to the owning user.
    /// Works for both user-uploaded files and LLM-saved artifacts.
    pub async fn get_file_by_id(
        &self,
        file_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ConversationFile>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT id as file_id, filename, user_id, mime_type, created_at
            FROM files
            WHERE id = $1 AND user_id = $2
            "#,
            file_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(row.map(|r| ConversationFile {
            file_id: r.file_id,
            filename: r.filename,
            user_id: r.user_id,
            mime_type: r.mime_type,
            created_at: r.created_at,
        }))
    }

    /// Get the owner's user_id for a conversation.
    pub async fn get_conversation_user_id(
        &self,
        conversation_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let user_id = sqlx::query_scalar!(
            r#"SELECT user_id as "user_id!" FROM conversations WHERE id = $1"#,
            conversation_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

        Ok(user_id)
    }

    /// Idempotent upsert of the code sandbox as a built-in system MCP server.
    pub async fn upsert_builtin_server(&self, server_id: Uuid, url: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_servers (
                id, name, display_name, description,
                transport_type, url, headers, args, environment_variables,
                timeout_seconds, enabled, is_system, is_built_in,
                supports_sampling, usage_mode, max_concurrent_sessions
            )
            VALUES (
                $1, 'code-sandbox', 'Code Sandbox',
                'Built-in code execution sandbox (bwrap). Runs Python, R, and Node.js securely.',
                'http', $2, '{}', '[]', '{}',
                620, true, true, true,
                false, 'auto', 1
            )
            ON CONFLICT (id) DO UPDATE SET
                url = EXCLUDED.url,
                display_name = EXCLUDED.display_name,
                description = EXCLUDED.description,
                enabled = EXCLUDED.enabled,
                is_built_in = EXCLUDED.is_built_in
            "#,
            server_id,
            url,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        // Assign to the default user group so users can see and use it.
        sqlx::query!(
            r#"
            INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
            SELECT g.id, $1
            FROM groups g
            WHERE g.is_default = true
            ON CONFLICT DO NOTHING
            "#,
            server_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Idempotent upsert of the code sandbox tools into every user's auto_approved_tools defaults.
    pub async fn upsert_builtin_user_defaults(&self, server_id: Uuid) -> Result<(), AppError> {
        let server_id_str = server_id.to_string();
        let tools_entry = serde_json::json!({
            "server_id": server_id_str,
            "tools": [
                "execute_command", "read_file", "write_file", "edit_file",
                "list_files", "get_resource_link"
            ]
        });

        sqlx::query!(
            r#"
            INSERT INTO user_mcp_defaults (user_id, approval_mode, auto_approved_tools, disabled_servers)
            SELECT id, 'manual_approve', '[]'::jsonb, '[]'::jsonb
            FROM users
            ON CONFLICT (user_id) DO NOTHING
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        sqlx::query!(
            r#"
            UPDATE user_mcp_defaults
            SET auto_approved_tools = (
                COALESCE(
                    (SELECT jsonb_agg(entry)
                     FROM jsonb_array_elements(
                         CASE WHEN jsonb_typeof(auto_approved_tools) = 'array'
                              THEN auto_approved_tools ELSE '[]'::jsonb END
                     ) AS entry
                     WHERE (entry->>'server_id') != $1),
                    '[]'::jsonb
                ) || jsonb_build_array($2::jsonb)
            ),
            updated_at = NOW()
            "#,
            server_id_str,
            tools_entry as serde_json::Value,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }
}
