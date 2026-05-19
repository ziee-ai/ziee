//! User MCP defaults repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::extensions::mcp::approval::models::{
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

/// Upsert MCP defaults for a user
/// `auto_approved_tools`: None = preserve existing DB value; Some(tools) = overwrite
pub async fn upsert_user_defaults(
    pool: &PgPool,
    user_id: Uuid,
    approval_mode: ApprovalMode,
    auto_approved_tools: Option<&[AutoApprovedServer]>,
    disabled_servers: &[DisabledServer],
    loop_settings: &LoopSettings,
) -> Result<UserMcpDefaults, AppError> {
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
        VALUES ($1, $2, COALESCE($3, '[]'::jsonb), $4, $5)
        ON CONFLICT (user_id)
        DO UPDATE SET
            approval_mode = EXCLUDED.approval_mode,
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
        approval_mode.to_string(),
        auto_approved_tools_json,
        disabled_servers_json,
        loop_settings_json
    )
    .fetch_one(pool)
    .await?;

    Ok(defaults)
}

/// Delete MCP defaults for a user
#[allow(dead_code)]
pub async fn delete_user_defaults(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM user_mcp_defaults
        WHERE user_id = $1
        "#,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
