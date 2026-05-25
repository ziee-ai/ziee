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
    modules::{
        permissions::{RequirePermissions, with_permission},
        user::models::Group,
    },
};

use super::super::{
    client::manager::McpSessionManager,
    permissions::*,
    runtime_types::*,
};

// =====================================================
// Helper Functions
// =====================================================

/// Check if user has admin-level access to MCP servers.
///
/// Admins (`user.is_admin = true`) always pass. Group membership
/// passes ONLY for the four defined `mcp_servers_admin::*` permissions
/// (read/create/edit/delete) via `check_permission_union`, which
/// already understands wildcard `*` and hierarchical `mcp_servers_admin::*`.
///
/// The previous implementation used `starts_with("mcp_servers_admin::")`
/// which:
///   1. Missed root admins because the extractor returns `groups: vec![]`
///      for `is_admin` users — closes 02-permissions F-06.
///   2. Was overly broad — any future permission named
///      `mcp_servers_admin::foo` (e.g. a low-priv `dry_run`) would
///      silently grant full admin bypass — closes 02-permissions F-07.
fn has_admin_access(user: &crate::modules::user::models::User, groups: &[Group]) -> bool {
    use crate::modules::permissions::checker::check_permission_union;
    if user.is_admin {
        return true;
    }
    const MCP_ADMIN_PERMISSIONS: &[&str] = &[
        "mcp_servers_admin::read",
        "mcp_servers_admin::create",
        "mcp_servers_admin::edit",
        "mcp_servers_admin::delete",
    ];
    MCP_ADMIN_PERMISSIONS
        .iter()
        .any(|p| check_permission_union(user, groups, p))
}

// =====================================================
// Handlers
// =====================================================

#[debug_handler]
pub async fn list_server_tools(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ListToolsResponse>> {
    // Check if user has access to this server
    // Admins with mcp_servers_admin::* permissions bypass access control
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;

        if !has_access {
            return Err(AppError::forbidden(
                "USER_NO_ACCESS",
                "You do not have access to this server"
            )
            .into());
        }
    }

    // Get or create session
    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;

    // List tools
    let mut session = session.write().await;
    let tools = session.list_tools().await?;

    Ok((StatusCode::OK, Json(ListToolsResponse { tools })))
}

#[debug_handler]
pub async fn call_server_tool(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    Json(request): Json<CallToolRequest>,
) -> ApiResult<Json<CallToolResponse>> {
    // Check if user has access to this server
    // Admins with mcp_servers_admin::* permissions bypass access control
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;

        if !has_access {
            return Err(AppError::forbidden(
                "USER_NO_ACCESS",
                "You do not have access to this server"
            )
            .into());
        }
    }

    // Get session
    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;

    // Call tool
    let mut session = session.write().await;
    let result = session.call_tool(&tool_name, request.arguments, None, None, None).await?;

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
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ListResourcesResponse>> {
    // Check if user has access to this server
    // Admins with mcp_servers_admin::* permissions bypass access control
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;

        if !has_access {
            return Err(AppError::forbidden(
                "USER_NO_ACCESS",
                "You do not have access to this server"
            )
            .into());
        }
    }

    // Get session
    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;

    // List resources
    let mut session = session.write().await;
    let resources = session.list_resources().await?;

    Ok((StatusCode::OK, Json(ListResourcesResponse { resources })))
}

#[debug_handler]
pub async fn read_server_resource(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
    Json(request): Json<ReadResourceRequest>,
) -> ApiResult<Json<ReadResourceResponse>> {
    // Check if user has access to this server
    // Admins with mcp_servers_admin::* permissions bypass access control
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;

        if !has_access {
            return Err(AppError::forbidden(
                "USER_NO_ACCESS",
                "You do not have access to this server"
            )
            .into());
        }
    }

    // Get session
    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;

    // Read resource
    let mut session = session.write().await;
    let content = session.read_resource(&request.uri).await?;

    Ok((StatusCode::OK, Json(ReadResourceResponse { content })))
}

#[debug_handler]
pub async fn disconnect_server(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<()>> {
    // Check if user has access to this server
    // Admins with mcp_servers_admin::* permissions bypass access control
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;

        if !has_access {
            return Err(AppError::forbidden(
                "USER_NO_ACCESS",
                "You do not have access to this server"
            )
            .into());
        }
    }

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
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

pub fn call_server_tool_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.callTool")
        .tag("MCP Servers - Runtime")
        .summary("Call MCP server tool")
        .description("Execute a tool on an MCP server")
        .response::<200, Json<CallToolResponse>>()
        .response_with::<404, (), _>(|res| res.description("Server or tool not found"))
}

pub fn list_server_resources_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.listResources")
        .tag("MCP Servers - Runtime")
        .summary("List MCP server resources")
        .description("List resources available from an MCP server")
        .response::<200, Json<ListResourcesResponse>>()
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

pub fn read_server_resource_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.readResource")
        .tag("MCP Servers - Runtime")
        .summary("Read MCP server resource")
        .description("Read the contents of a resource from an MCP server")
        .response::<200, Json<ReadResourceResponse>>()
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
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

// =====================================================
// Prompts (MCP spec § server/prompts)
// =====================================================

#[debug_handler]
pub async fn list_server_prompts(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ListPromptsResponse>> {
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;
        if !has_access {
            return Err(AppError::forbidden("USER_NO_ACCESS", "You do not have access to this server").into());
        }
    }

    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;
    let mut session = session.write().await;
    let prompts = session.list_prompts().await?;

    Ok((StatusCode::OK, Json(ListPromptsResponse { prompts })))
}

#[debug_handler]
pub async fn get_server_prompt(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
    Json(request): Json<GetPromptRequest>,
) -> ApiResult<Json<GetPromptResponse>> {
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;
        if !has_access {
            return Err(AppError::forbidden("USER_NO_ACCESS", "You do not have access to this server").into());
        }
    }

    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;
    let mut session = session.write().await;
    let prompt = session.get_prompt(&request.name, request.arguments).await?;

    Ok((StatusCode::OK, Json(GetPromptResponse { prompt })))
}

pub fn list_server_prompts_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.listPrompts")
        .tag("MCP Servers - Runtime")
        .summary("List MCP server prompts")
        .description("List prompt templates available from an MCP server")
        .response::<200, Json<ListPromptsResponse>>()
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

pub fn get_server_prompt_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.getPrompt")
        .tag("MCP Servers - Runtime")
        .summary("Get rendered MCP prompt")
        .description("Render a prompt template with the given arguments")
        .response::<200, Json<GetPromptResponse>>()
        .response_with::<404, (), _>(|res| res.description("Server or prompt not found"))
}

// =====================================================
// Ping (MCP spec § utilities/ping)
// =====================================================

#[debug_handler]
pub async fn ping_server(
    auth: RequirePermissions<(McpServersRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<PingResponse>> {
    if !has_admin_access(&auth.user, &auth.groups) {
        let has_access = Repos.mcp.can_user_access_server(auth.user.id, server_id).await?;
        if !has_access {
            return Err(AppError::forbidden("USER_NO_ACCESS", "You do not have access to this server").into());
        }
    }

    let session = session_manager.get_or_create_with_context(server_id, auth.user.id, None, None).await?;
    let mut session = session.write().await;
    session.ping().await?;

    Ok((StatusCode::OK, Json(PingResponse { ok: true })))
}

pub fn ping_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServerRuntime.ping")
        .tag("MCP Servers - Runtime")
        .summary("Ping MCP server")
        .description("Liveness check — verifies the server is reachable and responsive")
        .response::<200, Json<PingResponse>>()
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
