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
        // Log the full sqlx error server-side (it can include
        // constraint names, column names, datatypes — useful for
        // debugging). Return only the generic code to the client so
        // an authenticated caller can't fingerprint the schema.
        tracing::error!(error = ?e, "code_sandbox: database error");
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            "database error",
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
            WITH raw_refs AS (
                SELECT COALESCE(
                    (mc.content ->> 'file_id'),
                    (mc.content -> 'source' ->> 'file_id')
                ) AS file_id_str
                FROM conversations c
                JOIN branch_messages bm ON bm.branch_id = c.active_branch_id
                JOIN message_contents mc ON mc.message_id = bm.message_id
                WHERE c.id = $1
                  AND mc.content_type IN ('file_attachment', 'image')
                  AND (
                      mc.content ? 'file_id'
                      OR (mc.content -> 'source' ->> 'file_id') IS NOT NULL
                  )
            ),
            -- Defense: a malformed file_id in user-supplied JSON
            -- (chat messages can carry attacker-influenced content)
            -- would crash the unconditional ::uuid cast and break
            -- build_context for the conversation. Filter to
            -- well-formed UUID strings before casting; malformed
            -- entries are silently dropped (they wouldn't have
            -- matched a real `files` row anyway).
            attachment_refs AS (
                SELECT DISTINCT file_id_str::uuid AS file_id
                FROM raw_refs
                WHERE file_id_str ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
            ),
            -- Also include the PROJECT knowledge files of the project this
            -- conversation belongs to (if any) — so the sandbox sees the same
            -- effective file set as the chat (Track A manifest), not just
            -- attachments. Consistency gap fix.
            project_refs AS (
                SELECT pf.file_id
                FROM project_conversations pc
                JOIN project_files pf ON pf.project_id = pc.project_id
                WHERE pc.conversation_id = $1
            ),
            file_refs AS (
                SELECT file_id FROM attachment_refs
                UNION
                SELECT file_id FROM project_refs
            )
            SELECT DISTINCT
                f.id AS file_id,
                f.filename,
                f.user_id,
                f.mime_type,
                f.created_at
            FROM files f
            JOIN file_refs fr ON fr.file_id = f.id
            -- `f.id` tiebreaker keeps the order (and therefore the collision
            -- suffixing in build_bwrap_argv) deterministic + stable across calls
            -- even when several files share a created_at (bulk project uploads).
            ORDER BY f.created_at, f.id
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
    /// Critical contract: the `ON CONFLICT DO UPDATE SET` clause must
    /// ONLY refresh fields that are NOT admin-mutable. Unlike the
    /// `files`/`memory` zero-config built-ins (whose rows the
    /// `update_system_mcp_server` guard rejects), the `code_sandbox` row
    /// IS admin-editable: the admin UI lets operators tweak
    /// `display_name`, `description`, `timeout_seconds`, `usage_mode`,
    /// `max_concurrent_sessions`, and `enabled` via PATCH on the
    /// system-servers endpoint (`mcp/repository.rs:update_system_mcp_server`).
    /// Overwriting those on every boot would silently revert admin
    /// changes with no log line — confusing and operationally fragile.
    ///
    /// What we refresh on conflict:
    ///   - `is_system`, `is_built_in`: identity columns that cannot be
    ///     changed via UI; safe to assert each boot.
    ///   - `transport_type`, `url`: technically admin-editable but
    ///     editing them would break the built-in (the URL is the
    ///     loopback port; transport_type is always http). Refresh so
    ///     a port change in `server.port` actually takes effect.
    ///   - `supports_sampling`: capability flag, identity-shaped.
    ///   - `updated_at`: just a timestamp.
    ///
    /// What we DELIBERATELY OMIT (preserve admin's value on conflict):
    ///   - `enabled` — admin disable via UI must survive restart.
    ///   - `display_name`, `description` — cosmetic, admin-tunable.
    ///   - `timeout_seconds`, `usage_mode`, `max_concurrent_sessions` —
    ///     operational tunables admin may have raised/lowered.
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
                is_system = EXCLUDED.is_system,
                is_built_in = EXCLUDED.is_built_in,
                transport_type = EXCLUDED.transport_type,
                url = EXCLUDED.url,
                supports_sampling = EXCLUDED.supports_sampling,
                updated_at = NOW()
                -- DELIBERATELY OMITTED on conflict (admin-tunable):
                --   enabled, display_name, description,
                --   timeout_seconds, usage_mode, max_concurrent_sessions.
                -- Each is preserved if an admin set it via UI.
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
