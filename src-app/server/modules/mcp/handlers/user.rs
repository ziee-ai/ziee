// User MCP server handlers
// These handlers manage personal MCP servers owned by individual users

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
// User Handlers
// =====================================================

/// List user's accessible MCP servers (own + group-assigned system servers)
pub async fn list_accessible_servers(
    auth: RequirePermissions<(McpServersRead,)>,
    Query(params): Query<PaginationQuery>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<Json<McpServerListResponse>> {
    let (servers, total) =
        repository::list_accessible_mcp_servers(&pool, auth.user.id, params.page as i64, params.per_page as i64)
            .await?;

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
pub async fn create_user_server(
    auth: RequirePermissions<(McpServersCreate,)>,
    Extension(pool): Extension<PgPool>,
    Json(request): Json<CreateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::create_user_mcp_server(&pool, auth.user.id, request).await?;

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
pub async fn get_user_server(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::get_user_mcp_server(&pool, id, auth.user.id)
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
pub async fn update_user_server(
    auth: RequirePermissions<(McpServersEdit,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
    Json(request): Json<UpdateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let server = repository::update_user_mcp_server(&pool, id, auth.user.id, request).await?;

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
pub async fn delete_user_server(
    auth: RequirePermissions<(McpServersDelete,)>,
    Path(id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
) -> ApiResult<StatusCode> {
    repository::delete_user_mcp_server(&pool, id, auth.user.id).await?;

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
