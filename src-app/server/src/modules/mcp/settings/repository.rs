// Repository for the unified `mcp_settings` table.
//
// Exposed as `Repos.mcp_settings`. Used by both mcp/chat_extension
// (conversation scope) and mcp/project_extension (project scope) — the
// scope is just metadata around the payload at this layer.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::common::AppError;

use super::models::{McpScope, McpSettings, McpSettingsUpdate};

pub struct McpSettingsRepository {
    pool: PgPool,
}

/// Default approval mode applied when a scope has no row. Matches the
/// table's column default.
const DEFAULT_APPROVAL_MODE: &str = "manual_approve";

impl McpSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Look up the row for the given scope. Returns `None` if no row
    /// has been written yet (caller treats that as "use defaults").
    pub async fn get(&self, scope: McpScope) -> Result<Option<McpSettings>, AppError> {
        match scope {
            McpScope::Conversation(conv_id) => {
                let row = sqlx::query!(
                    r#"
                    SELECT id, conversation_id, project_id, user_id,
                           approval_mode, auto_approved_tools,
                           disabled_servers, loop_settings,
                           created_at, updated_at
                    FROM mcp_settings
                    WHERE conversation_id = $1
                    "#,
                    conv_id
                )
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::database_error)?;
                Ok(row.map(|r| McpSettings {
                    id: r.id,
                    scope: McpScope::Conversation(r.conversation_id.expect("FK set per CHECK")),
                    user_id: r.user_id,
                    approval_mode: r.approval_mode,
                    auto_approved_tools: r.auto_approved_tools,
                    disabled_servers: r.disabled_servers,
                    loop_settings: r.loop_settings,
                    created_at: chrono_from_ts(r.created_at),
                    updated_at: chrono_from_ts(r.updated_at),
                }))
            }
            McpScope::Project(project_id) => {
                let row = sqlx::query!(
                    r#"
                    SELECT id, conversation_id, project_id, user_id,
                           approval_mode, auto_approved_tools,
                           disabled_servers, loop_settings,
                           created_at, updated_at
                    FROM mcp_settings
                    WHERE project_id = $1
                    "#,
                    project_id
                )
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::database_error)?;
                Ok(row.map(|r| McpSettings {
                    id: r.id,
                    scope: McpScope::Project(r.project_id.expect("FK set per CHECK")),
                    user_id: r.user_id,
                    approval_mode: r.approval_mode,
                    auto_approved_tools: r.auto_approved_tools,
                    disabled_servers: r.disabled_servers,
                    loop_settings: r.loop_settings,
                    created_at: chrono_from_ts(r.created_at),
                    updated_at: chrono_from_ts(r.updated_at),
                }))
            }
        }
    }

    /// Synthesize a default-row in memory when no row exists yet.
    /// Lets callers avoid the `Option` branch for the common case.
    /// The synthesized row has a zero `id` (it's never persisted).
    pub async fn get_or_default(
        &self,
        scope: McpScope,
        user_id: Uuid,
    ) -> Result<McpSettings, AppError> {
        if let Some(row) = self.get(scope).await? {
            return Ok(row);
        }
        Ok(McpSettings {
            id: Uuid::nil(),
            scope,
            user_id,
            approval_mode: DEFAULT_APPROVAL_MODE.to_string(),
            auto_approved_tools: serde_json::json!([]),
            disabled_servers: serde_json::json!([]),
            loop_settings: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Insert OR update the row for the given scope. Each payload field
    /// is `Option`; `None` means "leave the existing value alone on
    /// update" (or use the column default on insert). `loop_settings`
    /// is `Option<Option<Value>>` (tri-state) so callers can distinguish
    /// "don't touch" from "explicitly clear".
    pub async fn upsert(
        &self,
        scope: McpScope,
        user_id: Uuid,
        update: McpSettingsUpdate,
    ) -> Result<McpSettings, AppError> {
        // Resolve effective payload by merging defaults + caller's overrides.
        // For UPDATE we need to keep existing values when the caller didn't
        // specify; do the merge in-memory by fetching first if needed.
        let existing = self.get(scope).await?;
        let approval_mode = update
            .approval_mode
            .or_else(|| existing.as_ref().map(|e| e.approval_mode.clone()))
            .unwrap_or_else(|| DEFAULT_APPROVAL_MODE.to_string());
        let auto_approved_tools = update
            .auto_approved_tools
            .or_else(|| existing.as_ref().map(|e| e.auto_approved_tools.clone()))
            .unwrap_or_else(|| serde_json::json!([]));
        let disabled_servers = update
            .disabled_servers
            .or_else(|| existing.as_ref().map(|e| e.disabled_servers.clone()))
            .unwrap_or_else(|| serde_json::json!([]));
        let loop_settings = match update.loop_settings {
            Some(v) => v,
            None => existing.as_ref().and_then(|e| e.loop_settings.clone()),
        };

        // Branch on scope first so each ON CONFLICT clause targets the
        // right unique constraint. The two query! invocations produce
        // different anonymous Record types, so each branch returns a
        // fully-constructed `McpSettings` directly (avoids the
        // "match arms have incompatible types" error from sharing a
        // single typed-row let-binding across branches).
        match scope {
            McpScope::Conversation(conv_id) => {
                let row = sqlx::query!(
                    r#"
                    INSERT INTO mcp_settings (
                        conversation_id, user_id,
                        approval_mode, auto_approved_tools, disabled_servers, loop_settings
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (conversation_id) DO UPDATE SET
                        user_id             = EXCLUDED.user_id,
                        approval_mode       = EXCLUDED.approval_mode,
                        auto_approved_tools = EXCLUDED.auto_approved_tools,
                        disabled_servers    = EXCLUDED.disabled_servers,
                        loop_settings       = EXCLUDED.loop_settings,
                        updated_at          = NOW()
                    RETURNING id, user_id, approval_mode, auto_approved_tools,
                              disabled_servers, loop_settings,
                              created_at, updated_at
                    "#,
                    conv_id,
                    user_id,
                    approval_mode,
                    auto_approved_tools,
                    disabled_servers,
                    loop_settings,
                )
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::database_error)?;
                Ok(McpSettings {
                    id: row.id,
                    scope,
                    user_id: row.user_id,
                    approval_mode: row.approval_mode,
                    auto_approved_tools: row.auto_approved_tools,
                    disabled_servers: row.disabled_servers,
                    loop_settings: row.loop_settings,
                    created_at: chrono_from_ts(row.created_at),
                    updated_at: chrono_from_ts(row.updated_at),
                })
            }
            McpScope::Project(proj_id) => {
                let row = sqlx::query!(
                    r#"
                    INSERT INTO mcp_settings (
                        project_id, user_id,
                        approval_mode, auto_approved_tools, disabled_servers, loop_settings
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (project_id) DO UPDATE SET
                        user_id             = EXCLUDED.user_id,
                        approval_mode       = EXCLUDED.approval_mode,
                        auto_approved_tools = EXCLUDED.auto_approved_tools,
                        disabled_servers    = EXCLUDED.disabled_servers,
                        loop_settings       = EXCLUDED.loop_settings,
                        updated_at          = NOW()
                    RETURNING id, user_id, approval_mode, auto_approved_tools,
                              disabled_servers, loop_settings,
                              created_at, updated_at
                    "#,
                    proj_id,
                    user_id,
                    approval_mode,
                    auto_approved_tools,
                    disabled_servers,
                    loop_settings,
                )
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::database_error)?;
                Ok(McpSettings {
                    id: row.id,
                    scope,
                    user_id: row.user_id,
                    approval_mode: row.approval_mode,
                    auto_approved_tools: row.auto_approved_tools,
                    disabled_servers: row.disabled_servers,
                    loop_settings: row.loop_settings,
                    created_at: chrono_from_ts(row.created_at),
                    updated_at: chrono_from_ts(row.updated_at),
                })
            }
        }
    }

    /// Copy the payload from `src` into a new row scoped to `dst`,
    /// inside the given transaction. Used by mcp's ProjectExtension on
    /// conversation-attach (project → conversation) and on
    /// project-duplicate (project → project). Silently no-op if src
    /// has no row (the caller treats that as "use defaults").
    pub async fn snapshot<'a>(
        &self,
        src: McpScope,
        dst: McpScope,
        user_id: Uuid,
        tx: &mut Transaction<'a, Postgres>,
    ) -> Result<bool, AppError> {
        // Two-step (read + write) so we have one INSERT path per dst
        // scope (different ON CONFLICT targets) without an N×M SQL
        // matrix. The read is in the same transaction so the snapshot
        // remains atomic with the project-conversations row insert
        // that triggered it.
        let src_row = match src {
            McpScope::Conversation(src_id) => sqlx::query!(
                r#"
                SELECT approval_mode, auto_approved_tools, disabled_servers, loop_settings
                FROM mcp_settings WHERE conversation_id = $1
                "#,
                src_id,
            )
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .map(|r| {
                (
                    r.approval_mode,
                    r.auto_approved_tools,
                    r.disabled_servers,
                    r.loop_settings,
                )
            }),
            McpScope::Project(src_id) => sqlx::query!(
                r#"
                SELECT approval_mode, auto_approved_tools, disabled_servers, loop_settings
                FROM mcp_settings WHERE project_id = $1
                "#,
                src_id,
            )
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .map(|r| {
                (
                    r.approval_mode,
                    r.auto_approved_tools,
                    r.disabled_servers,
                    r.loop_settings,
                )
            }),
        };

        let Some((approval_mode, auto_approved_tools, disabled_servers, loop_settings)) = src_row
        else {
            // No source row → nothing to snapshot. Caller treats absence
            // as "use defaults", which is the same behavior the
            // conversation chat-extension provides when no row exists
            // (the `get_or_default` path).
            return Ok(false);
        };

        let rows = match dst {
            McpScope::Conversation(dst_id) => sqlx::query!(
                r#"
                INSERT INTO mcp_settings (
                    conversation_id, user_id,
                    approval_mode, auto_approved_tools, disabled_servers, loop_settings
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (conversation_id) DO UPDATE SET
                    approval_mode       = EXCLUDED.approval_mode,
                    auto_approved_tools = EXCLUDED.auto_approved_tools,
                    disabled_servers    = EXCLUDED.disabled_servers,
                    loop_settings       = EXCLUDED.loop_settings,
                    updated_at          = NOW()
                "#,
                dst_id,
                user_id,
                approval_mode,
                auto_approved_tools,
                disabled_servers,
                loop_settings,
            )
            .execute(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .rows_affected(),
            McpScope::Project(dst_id) => sqlx::query!(
                r#"
                INSERT INTO mcp_settings (
                    project_id, user_id,
                    approval_mode, auto_approved_tools, disabled_servers, loop_settings
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (project_id) DO UPDATE SET
                    approval_mode       = EXCLUDED.approval_mode,
                    auto_approved_tools = EXCLUDED.auto_approved_tools,
                    disabled_servers    = EXCLUDED.disabled_servers,
                    loop_settings       = EXCLUDED.loop_settings,
                    updated_at          = NOW()
                "#,
                dst_id,
                user_id,
                approval_mode,
                auto_approved_tools,
                disabled_servers,
                loop_settings,
            )
            .execute(&mut **tx)
            .await
            .map_err(AppError::database_error)?
            .rows_affected(),
        };
        Ok(rows > 0)
    }
}

/// Convert sqlx's `time::OffsetDateTime` to `chrono::DateTime<Utc>` so the
/// rest of the codebase can use chrono (existing convention — see
/// `project/repository.rs` which does the same).
fn chrono_from_ts(ts: time::OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.unix_timestamp(), 0).unwrap_or_else(Utc::now)
}
