// Group MCP server assignment handlers
// These handlers manage MCP server assignments to user groups

use aide::transform::TransformOperation;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::ApiResult,
    modules::permissions::{with_permission, RequirePermissions},
};

use super::super::{
    models::ServerGroupsRequest,
    permissions::*,
    repository,
};

// =====================================================
// Group Assignment Handlers (Server-Centric)
// =====================================================

/// Get groups assigned to an MCP server
pub async fn get_server_groups(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<Vec<Uuid>>> {
    let group_ids = repository::get_server_groups(&pool, id).await?;

    Ok((StatusCode::OK, Json(group_ids)))
}

pub fn get_server_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerSystem.getServerGroups")
        .tag("MCP Servers - System")
        .summary("Get server's assigned groups")
        .description("Get groups assigned to an MCP server")
        .response::<200, Json<Vec<Uuid>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Assign MCP server to groups (replaces all assignments)
pub async fn assign_server_to_groups(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(request): Json<ServerGroupsRequest>,
) -> ApiResult<StatusCode> {
    repository::set_server_groups(&pool, id, request.group_ids).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn assign_server_to_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerSystem.assignServerToGroups")
        .tag("MCP Servers - System")
        .summary("Assign server to groups")
        .description("Assign MCP server to groups (replaces all assignments)")
        .response_with::<204, (), _>(|res| res.description("Server assigned successfully"))
        .response_with::<400, (), _>(|res| res.description("Bad request - only system servers can be assigned to groups"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Remove MCP server from group
pub async fn remove_server_from_group(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path((id, group_id)): Path<(Uuid, Uuid)>,
    State(pool): State<PgPool>,
) -> ApiResult<StatusCode> {
    repository::remove_mcp_server_from_group(&pool, group_id, id).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn remove_server_from_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerSystem.removeServerFromGroup")
        .tag("MCP Servers - System")
        .summary("Remove server from group")
        .description("Remove an MCP server from a specific group")
        .response_with::<204, (), _>(|res| res.description("Server removed successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server assignment not found"))
}
