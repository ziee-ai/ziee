//! User MCP defaults repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::extensions::mcp::approval::models::{
    ApprovalMode, AutoApprovedServer, DisabledServer,
};

use super::models::UserMcpDefaults;

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
            approval_mode, auto_approved_tools, disabled_servers,
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
pub async fn upsert_user_defaults(
    pool: &PgPool,
    user_id: Uuid,
    approval_mode: ApprovalMode,
    auto_approved_tools: &[AutoApprovedServer],
    disabled_servers: &[DisabledServer],
) -> Result<UserMcpDefaults, AppError> {
    let auto_approved_tools_json = serde_json::to_value(auto_approved_tools)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize auto_approved_tools: {}", e)))?;
    let disabled_servers_json = serde_json::to_value(disabled_servers)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize disabled_servers: {}", e)))?;

    let defaults = sqlx::query_as!(
        UserMcpDefaults,
        r#"
        INSERT INTO user_mcp_defaults (
            user_id, approval_mode, auto_approved_tools, disabled_servers
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_id)
        DO UPDATE SET
            approval_mode = EXCLUDED.approval_mode,
            auto_approved_tools = EXCLUDED.auto_approved_tools,
            disabled_servers = EXCLUDED.disabled_servers,
            updated_at = NOW()
        RETURNING
            id, user_id,
            approval_mode, auto_approved_tools, disabled_servers,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        user_id,
        approval_mode.to_string(),
        auto_approved_tools_json,
        disabled_servers_json
    )
    .fetch_one(pool)
    .await?;

    Ok(defaults)
}

/// Delete MCP defaults for a user
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
