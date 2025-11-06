// Group MCP server assignment handlers
// These handlers manage MCP server assignments to user groups

use aide::transform::TransformOperation;
use axum::{
    extract::Path,
    http::StatusCode,
    Extension, Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::ApiResult,
    modules::permissions::{with_permission, RequirePermissions},
};

use super::super::{
    models::GroupMcpServersRequest,
    permissions::*,
    repository,
};

// =====================================================
// Group Assignment Handlers
// =====================================================

/// Get MCP servers assigned to a group
pub async fn get_group_servers(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Path(group_id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<Json<Vec<Uuid>>> {
    let server_ids = repository::get_group_mcp_servers(&pool, group_id).await?;

    Ok((StatusCode::OK, Json(server_ids)))
}

pub fn get_group_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerAdmin.getGroupServers")
        .tag("Admin - MCP Servers")
        .summary("Get group's MCP servers")
        .description("Get MCP servers assigned to a group")
        .response::<200, Json<Vec<Uuid>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Set MCP servers for a group (replaces all assignments)
pub async fn set_group_servers(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path(group_id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
    Json(request): Json<GroupMcpServersRequest>,
) -> ApiResult<StatusCode> {
    repository::set_group_mcp_servers(&pool, group_id, request.server_ids).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn set_group_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerAdmin.setGroupServers")
        .tag("Admin - MCP Servers")
        .summary("Set group's MCP servers")
        .description("Set MCP servers for a group (replaces all assignments)")
        .response_with::<204, (), _>(|res| res.description("Servers assigned successfully"))
        .response_with::<400, (), _>(|res| res.description("Bad request - only system servers can be assigned"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Remove MCP server from group
pub async fn remove_group_server(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path((group_id, server_id)): Path<(Uuid, Uuid)>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<StatusCode> {
    repository::remove_mcp_server_from_group(&pool, group_id, server_id).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn remove_group_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerAdmin.removeGroupServer")
        .tag("Admin - MCP Servers")
        .summary("Remove MCP server from group")
        .description("Remove an MCP server from a group")
        .response_with::<204, (), _>(|res| res.description("Server removed successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server assignment not found"))
}
