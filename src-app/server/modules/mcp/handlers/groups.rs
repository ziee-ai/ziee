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
    models::{GroupSystemServersResponse, ServerGroupsRequest, UpdateGroupSystemServersRequest},
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

// =====================================================
// Group Assignment Handlers (Group-Centric, for UI Widgets)
// =====================================================

/// Get all system MCP servers assigned to a group
pub async fn get_group_system_servers(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Path(group_id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> ApiResult<Json<GroupSystemServersResponse>> {
    let servers = repository::get_system_servers_for_group(&pool, group_id)
        .await
        .map_err(|e| {
            eprintln!("Failed to get system servers for group {}: {}", group_id, e);
            crate::common::AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupSystemServersResponse { servers })))
}

pub fn get_group_system_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("Group.getSystemServers")
        .tag("Admin - Groups")
        .summary("Get all system servers assigned to a group")
        .description("Get all system MCP servers assigned to a group (for UI widgets)")
        .response::<200, Json<GroupSystemServersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Bulk update system MCP servers for a group (requires mcp_servers::admin_edit permission)
/// Atomically updates server assignments - adds new servers and removes unspecified ones
pub async fn update_group_system_servers(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path(group_id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(request): Json<UpdateGroupSystemServersRequest>,
) -> ApiResult<Json<GroupSystemServersResponse>> {
    use std::collections::HashSet;

    // Get current assignments
    let current = repository::get_system_servers_for_group(&pool, group_id)
        .await
        .map_err(|e| {
            eprintln!("Failed to get current servers for group {}: {}", group_id, e);
            crate::common::AppError::internal_error("Database operation failed")
        })?;

    let current_ids: HashSet<Uuid> = current.iter().map(|s| s.id).collect();
    let new_ids: HashSet<Uuid> = request.server_ids.iter().copied().collect();

    // Calculate diff
    let to_add: Vec<Uuid> = new_ids.difference(&current_ids).copied().collect();
    let to_remove: Vec<Uuid> = current_ids.difference(&new_ids).copied().collect();

    // Apply changes - remove first, then add
    for server_id in to_remove {
        repository::remove_mcp_server_from_group(&pool, group_id, server_id)
            .await
            .map_err(|e| {
                eprintln!("Failed to remove server {} from group {}: {}", server_id, group_id, e);
                crate::common::AppError::internal_error("Database operation failed")
            })?;
    }

    for server_id in to_add {
        repository::assign_mcp_server_to_group(&pool, group_id, server_id)
            .await
            .map_err(|e| {
                eprintln!("Failed to assign server {} to group {}: {}", server_id, group_id, e);
                crate::common::AppError::internal_error("Database operation failed")
            })?;
    }

    // Return updated list
    let servers = repository::get_system_servers_for_group(&pool, group_id)
        .await
        .map_err(|e| {
            eprintln!("Failed to get updated servers for group {}: {}", group_id, e);
            crate::common::AppError::internal_error("Database operation failed")
        })?;

    Ok((StatusCode::OK, Json(GroupSystemServersResponse { servers })))
}

pub fn update_group_system_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("Group.updateSystemServers")
        .tag("Admin - Groups")
        .summary("Update system servers assigned to a group")
        .description("Atomically updates system server assignments. Adds new servers and removes unspecified ones.")
        .response::<200, Json<GroupSystemServersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
