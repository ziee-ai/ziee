use axum::http::StatusCode;
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

    fn db_err(e: sqlx::Error) -> AppError {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            format!("code_sandbox DB error: {e}"),
        )
    }

    /// Resolve the user who owns the given conversation.
    /// Returns Ok(None) when the conversation does not exist.
    pub async fn get_conversation_user_id(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM conversations WHERE id = $1",
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::db_err)?;
        Ok(row.map(|r| r.0))
    }

    /// Files attached to the conversation's *active* branch. Walks
    /// `conversations.active_branch_id → branch_messages → messages →
    /// message_contents` and extracts `file_id` from the JSONB content
    /// for `content_type IN ('file_attachment', 'image')`.
    ///
    /// The two content types store file_id differently:
    ///   - file_attachment: `content->>'file_id'` (direct)
    ///   - image:           `content->'source'->>'file_id'` (nested under source.type='file')
    /// We coalesce both paths.
    pub async fn get_conversation_files(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<ConversationFile>, AppError> {
        // Use a CTE to extract the file_id from either JSON shape, then
        // join to `files` for the canonical filename/mime_type/etc.
        // De-dup by file_id (a file referenced from multiple messages
        // surfaces once).
        let rows: Vec<ConversationFile> = sqlx::query_as::<_, ConversationFile>(
            r#"
            WITH file_refs AS (
                SELECT DISTINCT COALESCE(
                    (mc.content ->> 'file_id'),
                    (mc.content -> 'source' ->> 'file_id')
                )::uuid AS file_id
                FROM conversations c
                JOIN branch_messages bm ON bm.branch_id = c.active_branch_id
                JOIN message_contents mc ON mc.message_id = bm.message_id
                WHERE c.id = $1
                  AND mc.content_type IN ('file_attachment', 'image')
                  AND (
                      mc.content ? 'file_id'
                      OR (mc.content -> 'source' ->> 'file_id') IS NOT NULL
                  )
            )
            SELECT
                f.id AS file_id,
                f.filename,
                f.user_id,
                f.mime_type,
                f.created_at
            FROM files f
            JOIN file_refs fr ON fr.file_id = f.id
            ORDER BY f.created_at
            "#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Self::db_err)?;
        Ok(rows)
    }

    /// Fetch a single file by id, scoped to the user that owns it.
    /// Foreign-attachment access is denied at query time (returns None).
    pub async fn get_file_by_id(
        &self,
        file_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ConversationFile>, AppError> {
        let row: Option<ConversationFile> = sqlx::query_as::<_, ConversationFile>(
            r#"
            SELECT id AS file_id, filename, user_id, mime_type, created_at
            FROM files
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(file_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::db_err)?;
        Ok(row)
    }

    /// Idempotent upsert of the built-in sandbox MCP server row.
    ///
    /// Critical contract (validated by `tests/code_sandbox/integration/mcp_built_in_protection_test.rs`):
    /// the `ON CONFLICT DO UPDATE SET` clause must NOT include `enabled`,
    /// so admin-driven disable via the UI survives process restart.
    /// All other mutable fields (display_name, description, url,
    /// transport-related columns) DO get refreshed on every boot.
    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(Self::db_err)?;

        sqlx::query(
            r#"
            INSERT INTO mcp_servers (
                id, user_id, name, display_name, description,
                enabled, is_system, is_built_in,
                transport_type, url, headers,
                timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                created_at, updated_at
            ) VALUES (
                $1, NULL, 'code_sandbox', 'Code Sandbox',
                'Built-in bwrap-isolated code execution sandbox',
                true, true, true,
                'http', $2, '{}'::jsonb,
                620, false, 'auto', 1,
                NOW(), NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                display_name = EXCLUDED.display_name,
                description = EXCLUDED.description,
                is_system = EXCLUDED.is_system,
                is_built_in = EXCLUDED.is_built_in,
                transport_type = EXCLUDED.transport_type,
                url = EXCLUDED.url,
                timeout_seconds = EXCLUDED.timeout_seconds,
                supports_sampling = EXCLUDED.supports_sampling,
                usage_mode = EXCLUDED.usage_mode,
                max_concurrent_sessions = EXCLUDED.max_concurrent_sessions,
                updated_at = NOW()
                -- DELIBERATELY OMITTED: enabled. Admin disable persists across reboot.
            "#,
        )
        .bind(server_id)
        .bind(loopback_url)
        .execute(&mut *tx)
        .await
        .map_err(Self::db_err)?;

        // Attach to the default Users group (idempotent — primary key
        // composite of (group_id, mcp_server_id)).
        sqlx::query(
            r#"
            INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
            SELECT g.id, $1
            FROM groups g
            WHERE g.is_default = true AND g.is_system = true
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(server_id)
        .execute(&mut *tx)
        .await
        .map_err(Self::db_err)?;

        tx.commit().await.map_err(Self::db_err)?;
        Ok(())
    }
}
