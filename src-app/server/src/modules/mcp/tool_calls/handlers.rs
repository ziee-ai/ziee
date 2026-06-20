//! REST handlers for the MCP tool-call history (`/api/mcp/tool-calls`).
//!
//! Mirrors `handlers/user.rs::list_accessible_servers`: gated on
//! `McpServersRead` (held by every Users-group member), owner-scoped on
//! `auth.user.id`. Cross-user single-row access returns 404 (MCP convention).

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::super::permissions::McpServersRead;
use super::models::{McpToolCall, McpToolCallListResponse};

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

/// Query params for the tool-call history list.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListToolCallsQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter to a single MCP server.
    #[serde(default)]
    pub server_id: Option<Uuid>,
    /// Filter to a single conversation.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    /// Filter by built-in vs external servers (e.g. `false` to hide built-ins).
    #[serde(default)]
    pub is_built_in: Option<bool>,
}

/// GET /api/mcp/tool-calls — the caller's own tool-call history, newest-first.
#[debug_handler]
pub async fn list_tool_calls(
    auth: RequirePermissions<(McpServersRead,)>,
    Query(params): Query<ListToolCallsQuery>,
) -> ApiResult<Json<McpToolCallListResponse>> {
    let response = Repos
        .mcp
        .list_tool_calls(
            auth.user.id,
            params.server_id,
            params.conversation_id,
            params.is_built_in,
            params.page.max(1),
            params.per_page,
        )
        .await?;
    Ok((StatusCode::OK, Json(response)))
}

pub fn list_tool_calls_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpToolCall.list")
        .tag("MCP Servers - Tool Calls")
        .summary("List MCP tool-call history")
        .description(
            "List the caller's own MCP tool-call invocations, newest-first. \
             Optional `server_id` / `conversation_id` filters. Owner-scoped.",
        )
        .response::<200, Json<McpToolCallListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/mcp/tool-calls/{id} — one tool-call row (404 if not owned).
#[debug_handler]
pub async fn get_tool_call(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<McpToolCall>> {
    // Owner-scoped in SQL: a row owned by another user comes back None → 404.
    let row = Repos
        .mcp
        .get_tool_call(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Tool call"))?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_tool_call_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpToolCall.get")
        .tag("MCP Servers - Tool Calls")
        .summary("Get an MCP tool-call record")
        .description(
            "Fetch a single tool-call record by id. Owner-scoped: a row owned \
             by another user returns 404.",
        )
        .response::<200, Json<McpToolCall>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Not found"))
}
