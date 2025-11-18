// MCP runtime handlers
// Handles MCP server runtime operations (tools, resources, connections)

use aide::transform::TransformOperation;
use axum::{
    debug_handler,
    extract::Path,
    http::StatusCode,
    Extension,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::{
    client::manager::McpSessionManager,
    permissions::*,
    repository::McpRepository,
    runtime_types::*,
};

// =====================================================
// Handlers
// =====================================================

#[debug_handler]
pub async fn list_server_tools(
    _auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ListToolsResponse>> {
    // Verify server exists and user can access it
    // For now, we'll check if server exists
    // TODO: Add proper user access check when user context is available
    let repo = McpRepository::new(Repos.pool().clone());
    repo.get_system_server(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Get or create session
    let session = session_manager.get_or_create(server_id).await?;

    // List tools
    let mut session = session.write().await;
    let tools = session.list_tools().await?;

    Ok((StatusCode::OK, Json(ListToolsResponse { tools })))
}

#[debug_handler]
pub async fn call_server_tool(
    _auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    Json(request): Json<CallToolRequest>,
) -> ApiResult<Json<CallToolResponse>> {
    // Verify server exists
    let repo = McpRepository::new(Repos.pool().clone());
    repo.get_system_server(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Get session
    let session = session_manager.get_or_create(server_id).await?;

    // Call tool
    let mut session = session.write().await;
    let result = session.call_tool(&tool_name, request.arguments).await?;

    Ok((
        StatusCode::OK,
        Json(CallToolResponse {
            content: result.content,
            is_error: result.is_error,
        }),
    ))
}

#[debug_handler]
pub async fn list_server_resources(
    _auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ListResourcesResponse>> {
    // Verify server exists
    let repo = McpRepository::new(Repos.pool().clone());
    repo.get_system_server(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Get session
    let session = session_manager.get_or_create(server_id).await?;

    // List resources
    let mut session = session.write().await;
    let resources = session.list_resources().await?;

    Ok((StatusCode::OK, Json(ListResourcesResponse { resources })))
}

#[debug_handler]
pub async fn read_server_resource(
    _auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
    Json(request): Json<ReadResourceRequest>,
) -> ApiResult<Json<ReadResourceResponse>> {
    // Verify server exists
    let repo = McpRepository::new(Repos.pool().clone());
    repo.get_system_server(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Get session
    let session = session_manager.get_or_create(server_id).await?;

    // Read resource
    let mut session = session.write().await;
    let content = session.read_resource(&request.uri).await?;

    Ok((StatusCode::OK, Json(ReadResourceResponse { content })))
}

#[debug_handler]
pub async fn disconnect_server(
    _auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<()>> {
    // Verify server exists
    let repo = McpRepository::new(Repos.pool().clone());
    repo.get_system_server(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Close session
    session_manager.close(server_id).await?;

    Ok((StatusCode::OK, Json(())))
}

// =====================================================
// OpenAPI Documentation
// =====================================================

pub fn list_server_tools_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.listTools")
        .tag("MCP Servers - Runtime")
        .summary("List MCP server tools")
        .description("List tools available from an MCP server")
        .response::<200, Json<ListToolsResponse>>()
        .response_with::<403, (), _>(|res| {
            res.description("User does not have access to this server")
        })
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

pub fn call_server_tool_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.callTool")
        .tag("MCP Servers - Runtime")
        .summary("Call MCP server tool")
        .description("Execute a tool on an MCP server")
        .response::<200, Json<CallToolResponse>>()
        .response_with::<403, (), _>(|res| {
            res.description("User does not have access to this server")
        })
        .response_with::<404, (), _>(|res| res.description("Server or tool not found"))
}

pub fn list_server_resources_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.listResources")
        .tag("MCP Servers - Runtime")
        .summary("List MCP server resources")
        .description("List resources available from an MCP server")
        .response::<200, Json<ListResourcesResponse>>()
        .response_with::<403, (), _>(|res| {
            res.description("User does not have access to this server")
        })
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

pub fn read_server_resource_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.readResource")
        .tag("MCP Servers - Runtime")
        .summary("Read MCP server resource")
        .description("Read the contents of a resource from an MCP server")
        .response::<200, Json<ReadResourceResponse>>()
        .response_with::<403, (), _>(|res| {
            res.description("User does not have access to this server")
        })
        .response_with::<404, (), _>(|res| {
            res.description("Server or resource not found")
        })
}

pub fn disconnect_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.disconnect")
        .tag("MCP Servers - Runtime")
        .summary("Disconnect MCP server")
        .description("Disconnect from an MCP server and clean up the session")
        .response::<200, Json<()>>()
        .response_with::<403, (), _>(|res| {
            res.description("User does not have access to this server")
        })
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
