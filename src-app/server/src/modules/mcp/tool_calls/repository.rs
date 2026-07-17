//! DB access for `mcp_tool_calls` (free functions over `&PgPool`, mirroring
//! `workflow/repository.rs`).

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{CreateMcpToolCall, McpToolCall};

/// Insert one recorded tool call, returning the full row.
pub async fn insert_call(pool: &PgPool, req: CreateMcpToolCall) -> Result<McpToolCall, AppError> {
    let row = sqlx::query_as!(
        McpToolCall,
        r#"
        INSERT INTO mcp_tool_calls (
            server_id, server_name, is_built_in, user_id, conversation_id,
            branch_id, message_id, tool_use_id, tool_name, arguments_json,
            source, status, is_error, result_json, content_kinds, result_bytes,
            error_message, started_at, finished_at, duration_ms, workflow_run_id,
            review_classification
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)
        RETURNING
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        req.server_id,
        req.server_name,
        req.is_built_in,
        req.user_id,
        req.conversation_id,
        req.branch_id,
        req.message_id,
        req.tool_use_id,
        req.tool_name,
        req.arguments_json,
        req.source.as_str(),
        req.status.as_str(),
        req.is_error,
        req.result_json,
        &req.content_kinds,
        req.result_bytes,
        req.error_message,
        req.started_at,
        req.finished_at,
        req.duration_ms,
        req.workflow_run_id,
        req.review_classification,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// List a user's tool calls, newest-first, with optional server/conversation
/// filters. Returns `(rows, total)`; the handler derives `total_pages`.
pub async fn list_calls_for_user(
    pool: &PgPool,
    user_id: Uuid,
    server_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
    is_built_in: Option<bool>,
    page: i64,
    per_page: i64,
) -> Result<(Vec<McpToolCall>, i64), AppError> {
    let per_page = per_page.clamp(1, 200);
    let offset = (page - 1).max(0) * per_page;

    let rows = sqlx::query_as!(
        McpToolCall,
        r#"
        SELECT
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM mcp_tool_calls
        WHERE user_id = $1
          AND ($2::uuid IS NULL OR server_id = $2)
          AND ($3::uuid IS NULL OR conversation_id = $3)
          AND ($4::bool IS NULL OR is_built_in = $4)
        ORDER BY created_at DESC
        LIMIT $5 OFFSET $6
        "#,
        user_id,
        server_id,
        conversation_id,
        is_built_in,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let total = sqlx::query!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM mcp_tool_calls
        WHERE user_id = $1
          AND ($2::uuid IS NULL OR server_id = $2)
          AND ($3::uuid IS NULL OR conversation_id = $3)
          AND ($4::bool IS NULL OR is_built_in = $4)
        "#,
        user_id,
        server_id,
        conversation_id,
        is_built_in,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?
    .count;

    Ok((rows, total))
}

/// Fetch a single tool-call row by id, scoped to its owner. Ownership is
/// enforced in SQL (not just the handler) so a future caller can't leak a
/// cross-user row.
pub async fn find_call_for_user(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<McpToolCall>, AppError> {
    let row = sqlx::query_as!(
        McpToolCall,
        r#"
        SELECT
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM mcp_tool_calls
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Delete every row older than `cutoff` (the retention prune). Returns the
/// number of rows removed.
pub async fn prune_calls_older_than(
    pool: &PgPool,
    cutoff: time::OffsetDateTime,
) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM mcp_tool_calls WHERE created_at < $1"#,
        cutoff,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}
