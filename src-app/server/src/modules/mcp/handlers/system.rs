// System MCP server handlers
// These handlers manage system-wide MCP servers

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
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

use super::super::{
    connection_health::{enforce_on_create, enforce_on_update_transition, McpServerWithHealthWarning},
    events::McpServerEvent,
    models::McpServer,
    permissions::*,
    types::{CreateMcpServerRequest, McpServerListResponse, UpdateMcpServerRequest},
};

// =====================================================
// System Handlers
// =====================================================

/// Query params for the system MCP server list — extends pagination
/// with server-side `search` (name/display_name/description ILIKE)
/// and `status` (enabled / disabled, translated to a bool predicate).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSystemServersQuery {
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

/// List all system MCP servers
#[debug_handler]
pub async fn list_system_servers(
    _auth: RequirePermissions<(McpServersAdminRead,)>,
    Query(params): Query<ListSystemServersQuery>,
) -> ApiResult<Json<McpServerListResponse>> {
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let enabled = match params.status.as_deref() {
        Some("enabled") => Some(true),
        Some("disabled") => Some(false),
        _ => None,
    };

    let response = Repos
        .mcp
        .list_system_servers(
            params.page as i64,
            params.per_page as i64,
            search,
            enabled,
        )
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
    origin: SyncOrigin,
    Json(request): Json<CreateMcpServerRequest>,
) -> ApiResult<Json<McpServerWithHealthWarning>> {
    super::validate_sandbox_fields_create(true, &request)?;
    let hub_id = request.hub_id.clone();
    let server = Repos.mcp.create_system_server(request).await?;
    let server_id = server.id;

    // Hub install tracking from the drawer-prefilled flow (Install
    // for the system).
    if let Some(hub_id) = hub_id {
        crate::modules::hub::install_helpers::track_system_mcp_install(
            server_id,
            &hub_id,
        )
        .await?;
    }

    event_bus.emit_async(McpServerEvent::system_server_created(server.id));

    let server_id = server.id;
    // Same downgrade-on-probe-failure semantic as user creates —
    // built-in servers are skipped inside the helper.
    let wrapped = enforce_on_create(Repos.mcp.pool(), server, &event_bus).await?;

    // Cross-device sync: notify AFTER enforcement so peers refetch the final
    // (possibly probe-downgraded) state.
    sync_publish(SyncEntity::McpServerSystem, SyncAction::Create, server_id, Audience::perm::<McpServersAdminRead>(), origin.0);
    sync_publish(SyncEntity::UserMcpServer, SyncAction::Create, server_id, Audience::perm::<McpServersRead>(), origin.0);

    Ok((StatusCode::CREATED, Json(wrapped)))
}

pub fn create_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminCreate,)>(op)
        .id("McpServerSystem.create")
        .tag("MCP Servers - System")
        .summary("Create system MCP server")
        .description(
            "Create a new system MCP server configuration. Same \
             health-check-on-create semantic as the user create \
             endpoint — see `McpServer.create` for the contract on \
             `connection_warning` and the auto-downgrade-to-disabled \
             behavior.",
        )
        .response::<201, Json<McpServerWithHealthWarning>>()
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
    origin: SyncOrigin,
    Json(request): Json<UpdateMcpServerRequest>,
) -> ApiResult<Json<McpServer>> {
    let existing = Repos
        .mcp
        .get_system_server(id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;
    let prior_enabled = existing.enabled;
    super::validate_sandbox_fields_update(&existing, &request)?;

    let persisted = Repos.mcp.update_system_server(id, request).await?;
    event_bus.emit_async(McpServerEvent::system_server_updated(persisted.id));

    let server = enforce_on_update_transition(
        Repos.mcp.pool(),
        persisted,
        prior_enabled,
        &event_bus,
    )
    .await?;

    sync_publish(SyncEntity::McpServerSystem, SyncAction::Update, server.id, Audience::perm::<McpServersAdminRead>(), origin.0);
    sync_publish(SyncEntity::UserMcpServer, SyncAction::Update, server.id, Audience::perm::<McpServersRead>(), origin.0);

    Ok((StatusCode::OK, Json(server)))
}

pub fn update_system_server_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerSystem.update")
        .tag("MCP Servers - System")
        .summary("Update system MCP server")
        .description(
            "Update a system MCP server configuration. Same \
             enable-time health-check semantic as `McpServer.update` \
             — see that endpoint for the partial-save + \
             `MCP_ENABLE_FAILED_HEALTH_CHECK` contract.",
        )
        .response::<200, Json<McpServer>>()
        .response_with::<400, (), _>(|res| res.description("Bad request - validation failed (incl. enable-time health check failure with error_code `MCP_ENABLE_FAILED_HEALTH_CHECK`)"))
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
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    Repos.mcp.delete_system_server(id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(McpServerEvent::system_server_deleted(id)).await;

    sync_publish(SyncEntity::McpServerSystem, SyncAction::Delete, id, Audience::perm::<McpServersAdminRead>(), origin.0);
    sync_publish(SyncEntity::UserMcpServer, SyncAction::Delete, id, Audience::perm::<McpServersRead>(), origin.0);

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
