// User MCP server handlers
// These handlers manage personal MCP servers owned by individual users

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::EventBus,
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::{
    connection_health::{enforce_on_create, enforce_on_update_transition, McpServerWithHealthWarning},
    events::McpServerEvent,
    models::{McpServer, McpServerOAuthConfigResponse, SetMcpServerOAuthConfigRequest},
    permissions::*,
    types::{CreateMcpServerRequest, McpServerListResponse, UpdateMcpServerRequest},
};

// =====================================================
// User Handlers
// =====================================================

/// Query params for the user-accessible MCP server list.
///
/// Extends `PaginationQuery` with server-side filters that match
/// the UI's controls 1-to-1:
///   - `search` → ILIKE on name / display_name / description
///   - `status` → one of `enabled` | `disabled` | `system` | `user`
///                (translated here to enabled/is_system bool predicates)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAccessibleServersQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// List user's accessible MCP servers (own + group-assigned system servers)
#[debug_handler]
pub async fn list_accessible_servers(
    auth: RequirePermissions<(McpServersRead,)>,
    Query(params): Query<ListAccessibleServersQuery>,
) -> ApiResult<Json<McpServerListResponse>> {
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (enabled, is_system) = match params.status.as_deref() {
        Some("enabled") => (Some(true), None),
        Some("disabled") => (Some(false), None),
        Some("system") => (None, Some(true)),
        Some("user") => (None, Some(false)),
        _ => (None, None),
    };

    let response = Repos
        .mcp
        .list_accessible(
            auth.user.id,
            params.page as i64,
            params.per_page as i64,
            search,
            enabled,
            is_system,
        )
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
) -> ApiResult<Json<McpServerWithHealthWarning>> {
    super::validate_sandbox_fields_create(false, &request)?;
    let server = Repos.mcp.create_user_server(auth.user.id, request).await?;
    let server_id = server.id;

    // Emit creation event for other modules to react.
    event_bus.emit_async(McpServerEvent::user_server_created(server_id, auth.user.id));

    // If created with enabled=true, probe the connection. On
    // failure: downgrade to enabled=false (no data loss — the user's
    // config is preserved so they can fix + retry) and surface the
    // failure reason in the response so the UI can toast it.
    let wrapped = enforce_on_create(
        Repos.mcp.pool(),
        server,
        &event_bus,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(wrapped)))
}

pub fn create_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersCreate,)>(op)
        .id("McpServer.create")
        .tag("MCP Servers")
        .summary("Create user MCP server")
        .description(
            "Create a new personal MCP server configuration. When the \
             request asks for `enabled: true`, the server is probed \
             after creation; on probe failure the row is persisted with \
             `enabled: false` and `connection_warning` carries the \
             failure detail so the UI can surface it.",
        )
        .response::<201, Json<McpServerWithHealthWarning>>()
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
    // Capture the prior enabled state BEFORE the persist so we can
    // detect a false→true transition. The probe only fires on that
    // transition — flipping enabled false (or no change) skips the
    // health check entirely.
    let existing = Repos
        .mcp
        .get_user_server(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;
    let prior_enabled = existing.enabled;
    super::validate_sandbox_fields_update(&existing, &request)?;

    let persisted = Repos
        .mcp
        .update_user_server(id, auth.user.id, request)
        .await?;

    // Emit update event for other modules to react. Done BEFORE the
    // health check so listeners see the persisted state regardless
    // of whether the probe forces the row back to enabled=false.
    event_bus.emit_async(McpServerEvent::user_server_updated(
        persisted.id,
        auth.user.id,
    ));

    let server = enforce_on_update_transition(
        Repos.mcp.pool(),
        persisted,
        prior_enabled,
        &event_bus,
    )
    .await?;

    Ok((StatusCode::OK, Json(server)))
}

pub fn update_user_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersEdit,)>(op)
        .id("McpServer.update")
        .tag("MCP Servers")
        .summary("Update user MCP server")
        .description(
            "Update a user MCP server configuration. When the update \
             flips `enabled: false → true`, the persisted state is \
             probed; on probe failure the server is reverted to \
             `enabled: false` and the response is 400 \
             `MCP_ENABLE_FAILED_HEALTH_CHECK`. Other fields in the \
             same request DO persist regardless — the partial save is \
             intentional so concurrent edits aren't lost.",
        )
        .response::<200, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed (incl. enable-time health check failure with error_code `MCP_ENABLE_FAILED_HEALTH_CHECK`)"))
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

// =====================================================
// OAuth client_credentials config (Phase 4)
// External HTTP servers may require OAuth; these endpoints manage the
// per-server client_credentials config. The secret is write-only — it is
// never returned in any response.
// =====================================================

/// Ensure the caller owns the server (404 otherwise), returning it.
async fn owned_server(id: Uuid, user_id: Uuid) -> Result<McpServer, AppError> {
    Repos
        .mcp
        .get_user_server(id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))
}

/// Get a user server's OAuth config (secret omitted). `null` when unset.
#[debug_handler]
pub async fn get_server_oauth_config(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Option<McpServerOAuthConfigResponse>>> {
    owned_server(id, auth.user.id).await?;
    let cfg = Repos.mcp.get_oauth_config(id).await?.map(|c| c.to_response());
    Ok((StatusCode::OK, Json(cfg)))
}

pub fn get_server_oauth_config_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpServer.getOAuthConfig")
        .tag("MCP Servers")
        .summary("Get MCP server OAuth config")
        .description("Get a user MCP server's OAuth client_credentials config (the client secret is never returned)")
        .response::<200, Json<Option<McpServerOAuthConfigResponse>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Set (create or replace) a user server's OAuth config.
#[debug_handler]
pub async fn set_server_oauth_config(
    auth: RequirePermissions<(McpServersEdit,)>,
    Path(id): Path<Uuid>,
    Json(request): Json<SetMcpServerOAuthConfigRequest>,
) -> ApiResult<Json<McpServerOAuthConfigResponse>> {
    owned_server(id, auth.user.id).await?;
    if request.client_id.trim().is_empty() || request.client_secret.is_empty() {
        return Err(AppError::bad_request(
            "invalid_oauth_config",
            "client_id and client_secret are required",
        )
        .into());
    }
    let cfg = Repos.mcp.set_oauth_config(id, request).await?;
    Ok((StatusCode::OK, Json(cfg.to_response())))
}

pub fn set_server_oauth_config_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersEdit,)>(op)
        .id("McpServer.setOAuthConfig")
        .tag("MCP Servers")
        .summary("Set MCP server OAuth config")
        .description("Create or replace a user MCP server's OAuth client_credentials config")
        .response::<200, Json<McpServerOAuthConfigResponse>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// Delete a user server's OAuth config.
#[debug_handler]
pub async fn delete_server_oauth_config(
    auth: RequirePermissions<(McpServersEdit,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    owned_server(id, auth.user.id).await?;
    Repos.mcp.delete_oauth_config(id).await?;
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_server_oauth_config_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersEdit,)>(op)
        .id("McpServer.deleteOAuthConfig")
        .tag("MCP Servers")
        .summary("Delete MCP server OAuth config")
        .description("Remove a user MCP server's OAuth client_credentials config")
        .response_with::<204, (), _>(|res| res.description("OAuth config removed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
