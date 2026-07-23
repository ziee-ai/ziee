//! User MCP defaults repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::mcp::chat_extension::approval::models::{
    ApprovalMode, AutoApprovedServer, DisabledServer,
};

use super::models::{LoopSettings, UserMcpDefaults};

/// Get MCP defaults for a user
pub async fn get_user_defaults(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<UserMcpDefaults>, AppError> {
    let defaults = sqlx::query_as!(
        UserMcpDefaults,
        r#"
        SELECT
            id, user_id,
            approval_mode, auto_approved_tools, disabled_servers, loop_settings,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM user_mcp_defaults
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(defaults)
}

/// Upsert MCP defaults for a user.
///
/// Two tri-state fields, resolved by `COALESCE` INSIDE this one statement:
/// - `approval_mode`: None = preserve the existing row's value, or apply
///   [`ApprovalMode::default()`] when inserting; Some(mode) = set it explicitly.
/// - `auto_approved_tools`: None = preserve existing DB value; Some(tools) = overwrite.
///
/// The `approval_mode` None-case matters more here than anywhere else: the client
/// writes this row as a SIDE EFFECT of unrelated actions (removing an MCP server chip
/// on a new chat persists the server list here), and a mode pinned by such a write
/// becomes the fallback for EVERY future conversation of that user. Only the single
/// `user_id` row is ever touched.
pub async fn upsert_user_defaults(
    pool: &PgPool,
    user_id: Uuid,
    approval_mode: Option<ApprovalMode>,
    auto_approved_tools: Option<&[AutoApprovedServer]>,
    disabled_servers: &[DisabledServer],
    loop_settings: &LoopSettings,
) -> Result<UserMcpDefaults, AppError> {
    // None → SQL NULL, so COALESCE picks the preserve/default arm.
    let approval_mode_str: Option<String> = approval_mode.map(|m| m.to_string());
    let default_approval_mode = ApprovalMode::default().to_string();
    let auto_approved_tools_json = match auto_approved_tools {
        Some(tools) => serde_json::to_value(tools)
            .map_err(|e| AppError::internal_error(format!("Failed to serialize auto_approved_tools: {}", e)))?,
        None => serde_json::Value::Null,
    };
    let disabled_servers_json = serde_json::to_value(disabled_servers)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize disabled_servers: {}", e)))?;
    let loop_settings_json = serde_json::to_value(loop_settings)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize loop_settings: {}", e)))?;

    let defaults = sqlx::query_as!(
        UserMcpDefaults,
        r#"
        INSERT INTO user_mcp_defaults (
            user_id, approval_mode, auto_approved_tools, disabled_servers, loop_settings
        )
        VALUES ($1, COALESCE($2, $6), COALESCE($3, '[]'::jsonb), $4, $5)
        ON CONFLICT (user_id)
        DO UPDATE SET
            approval_mode = COALESCE($2, user_mcp_defaults.approval_mode),
            auto_approved_tools = COALESCE($3, user_mcp_defaults.auto_approved_tools),
            disabled_servers = EXCLUDED.disabled_servers,
            loop_settings = EXCLUDED.loop_settings,
            updated_at = NOW()
        RETURNING
            id, user_id,
            approval_mode, auto_approved_tools, disabled_servers, loop_settings,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        user_id,
        approval_mode_str,
        auto_approved_tools_json,
        disabled_servers_json,
        loop_settings_json,
        default_approval_mode
    )
    .fetch_one(pool)
    .await?;

    Ok(defaults)
}

