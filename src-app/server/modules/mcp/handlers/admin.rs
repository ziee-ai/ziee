// Admin MCP server handlers
// These handlers manage system-wide MCP servers

use aide::transform::TransformOperation;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    modules::permissions::{with_permission, RequirePermissions},
};

use super::super::{
    models::{CreateMcpServerRequest, McpServer, McpServerListResponse, UpdateMcpServerRequest},
    permissions::*,
    repository,
};

// =====================================================
// Admin Handlers
// =====================================================

/// List all system MCP servers
pub async fn list_system_servers(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Query(params): Query<PaginationQuery>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<Json<McpServerListResponse>> {
    let (servers, total) =
        repository::list_system_mcp_servers(&pool, params.page as i64, params.per_page as i64).await?;

    let total_pages = (total + params.per_page as i64 - 1) / params.per_page as i64;

    Ok((
        StatusCode::OK,
        Json(McpServerListResponse {
            servers,
            total,
            page: params.page as i64,
            per_page: params.per_page as i64,
            total_pages,
        }),
    ))
}

pub fn list_system_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerAdmin.list")
        .tag("Admin - MCP Servers")
        .summary("List system MCP servers")
        .description("List all system MCP servers")
        .response::<200, Json<McpServerListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Create a new system MCP server
pub async fn create_system_server(
    _auth: RequirePermissions<(McpServersAdminCreate,)>,
    Extension(pool): Extension<PgPool>,
    Json(request): Json<CreateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::create_system_mcp_server(&pool, request).await?;

    Ok((StatusCode::CREATED, Json(server)))
}

pub fn create_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminCreate,)>(op)
        .id("McpServerAdmin.create")
        .tag("Admin - MCP Servers")
        .summary("Create system MCP server")
        .description("Create a new system MCP server configuration")
        .response::<201, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Get system MCP server by ID
pub async fn get_system_server(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::get_system_mcp_server(&pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    Ok((StatusCode::OK, Json(server)))
}

pub fn get_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerAdmin.get")
        .tag("Admin - MCP Servers")
        .summary("Get system MCP server")
        .description("Get a system MCP server by ID")
        .response::<200, Json<McpServer>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Update system MCP server
pub async fn update_system_server(
    _auth: RequirePermissions<(McpServersAdminEdit,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
    Json(request): Json<UpdateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::update_system_mcp_server(&pool, id, request).await?;

    Ok((StatusCode::OK, Json(server)))
}

pub fn update_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerAdmin.update")
        .tag("Admin - MCP Servers")
        .summary("Update system MCP server")
        .description("Update a system MCP server configuration")
        .response::<200, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
        .response_with::<409, (), _>(|res| res.description("Server name already exists"))
}

/// Delete system MCP server
pub async fn delete_system_server(
    _auth: RequirePermissions<(McpServersAdminDelete,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<StatusCode> {
    repository::delete_system_mcp_server(&pool, id).await?;

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminDelete,)>(op)
        .id("McpServerAdmin.delete")
        .tag("Admin - MCP Servers")
        .summary("Delete system MCP server")
        .description("Delete a system MCP server configuration")
        .response_with::<204, (), _>(|res| res.description("Server deleted successfully"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
