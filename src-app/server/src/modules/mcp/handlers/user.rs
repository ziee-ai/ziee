// User MCP server handlers
// These handlers manage personal MCP servers owned by individual users

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
// User Handlers
// =====================================================

/// List user's accessible MCP servers (own + group-assigned system servers)
#[debug_handler]
pub async fn list_accessible_servers(
    auth: RequirePermissions<(McpServersRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<McpServerListResponse>> {
    let response = Repos
        .mcp
        .list_accessible(auth.user.id, params.page as i64, params.per_page as i64)
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn list_accessible_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServer.listAccessible")
        .tag("MCP Servers")
        .summary("List accessible MCP servers")
        .description("List user's own MCP servers and system servers assigned through groups")
        .response::<200, Json<McpServerListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Create a new user MCP server
#[debug_handler]
pub async fn create_user_server(
    auth: RequirePermissions<(McpServersCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos.mcp.create_user_server(auth.user.id, request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(McpServerEvent::user_server_created(server.id, auth.user.id));

    Ok((StatusCode::CREATED, Json(server)))
}

pub fn create_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersCreate,)>(op)
        .id("McpServer.create")
        .tag("MCP Servers")
        .summary("Create user MCP server")
        .description("Create a new personal MCP server configuration")
        .response::<201, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Get user MCP server by ID
#[debug_handler]
pub async fn get_user_server(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos
        .mcp
        .get_user_server(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    Ok((StatusCode::OK, Json(server)))
}

pub fn get_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServer.get")
        .tag("MCP Servers")
        .summary("Get user MCP server")
        .description("Get a user MCP server by ID")
        .response::<200, Json<McpServer>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Update user MCP server
#[debug_handler]
pub async fn update_user_server(
    auth: RequirePermissions<(McpServersEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = Repos
        .mcp
        .update_user_server(id, auth.user.id, request)
        .await?;

    // Emit update event for other modules to react
    event_bus.emit_async(McpServerEvent::user_server_updated(server.id, auth.user.id));

    Ok((StatusCode::OK, Json(server)))
}

pub fn update_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersEdit,)>(op)
        .id("McpServer.update")
        .tag("MCP Servers")
        .summary("Update user MCP server")
        .description("Update a user MCP server configuration")
        .response::<200, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Delete user MCP server
#[debug_handler]
pub async fn delete_user_server(
    auth: RequirePermissions<(McpServersDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    Repos.mcp.delete_user_server(id, auth.user.id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(McpServerEvent::user_server_deleted(id, auth.user.id)).await;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersDelete,)>(op)
        .id("McpServer.delete")
        .tag("MCP Servers")
        .summary("Delete user MCP server")
        .description("Delete a user MCP server configuration")
        .response_with::<204, (), _>(|res| res.description("Server deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
