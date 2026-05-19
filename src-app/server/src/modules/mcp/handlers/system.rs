// System MCP server handlers
// These handlers manage system-wide MCP servers

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    core::EventBus,
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::{
    events::McpServerEvent,
    models::McpServer,
    permissions::*,
    types::{CreateMcpServerRequest, McpServerListResponse, UpdateMcpServerRequest},
};

// =====================================================
// System Handlers
// =====================================================

/// List all system MCP servers
#[debug_handler]
pub async fn list_system_servers(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<McpServerListResponse>> {
    let response = Repos
        .mcp
        .list_system_servers(params.page as i64, params.per_page as i64)
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn list_system_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerSystem.list")
        .tag("MCP Servers - System")
        .summary("List system MCP servers")
        .description("List all system MCP servers")
        .response::<200, Json<McpServerListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Create a new system MCP server
#[debug_handler]
pub async fn create_system_server(
    _auth: RequirePermissions<(McpServersAdminCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos.mcp.create_system_server(request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(McpServerEvent::system_server_created(server.id));

    Ok((StatusCode::CREATED, Json(server)))
}

pub fn create_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminCreate,)>(op)
        .id("McpServerSystem.create")
        .tag("MCP Servers - System")
        .summary("Create system MCP server")
        .description("Create a new system MCP server configuration")
        .response::<201, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Get system MCP server by ID
#[debug_handler]
pub async fn get_system_server(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos
        .mcp
        .get_system_server(id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    Ok((StatusCode::OK, Json(server)))
}

pub fn get_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerSystem.get")
        .tag("MCP Servers - System")
        .summary("Get system MCP server")
        .description("Get a system MCP server by ID")
        .response::<200, Json<McpServer>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Update system MCP server
#[debug_handler]
pub async fn update_system_server(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos.mcp.update_system_server(id, request).await?;

    // Emit update event for other modules to react
    event_bus.emit_async(McpServerEvent::system_server_updated(server.id));

    Ok((StatusCode::OK, Json(server)))
}

pub fn update_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerSystem.update")
        .tag("MCP Servers - System")
        .summary("Update system MCP server")
        .description("Update a system MCP server configuration")
        .response::<200, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Delete system MCP server
#[debug_handler]
pub async fn delete_system_server(
    _auth: RequirePermissions<(McpServersAdminDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    Repos.mcp.delete_system_server(id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(McpServerEvent::system_server_deleted(id)).await;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminDelete,)>(op)
        .id("McpServerSystem.delete")
        .tag("MCP Servers - System")
        .summary("Delete system MCP server")
        .description("Delete a system MCP server configuration")
        .response_with::<204, (), _>(|res| res.description("Server deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
