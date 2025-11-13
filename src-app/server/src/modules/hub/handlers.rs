use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, extract::Query, http::StatusCode};

use crate::{
    common::{ApiResult, AppError},
    core::events::EventBus,
    modules::{
        llm_model::ModelParameters,
        permissions::{RequirePermissions, with_permission},
    },
};
use std::sync::Arc;

use super::{
    events::HubEvent,
    hub_manager::HubManager,
    models::{HubCategory, HubEntityType},
    permissions::*,
    types::*,
};

// =====================================================
// Route Handlers
// =====================================================

/// Get hub models with locale support and created_ids (system-wide)
#[debug_handler]
pub async fn get_hub_models(
    _auth: RequirePermissions<(HubModelsRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubModelsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created model IDs (system-wide, no user filter)
    let created_map = Repos.hub.get_created_model_ids().await?;

    // Merge created_ids into models
    let mut models = hub_data.models;
    for model in &mut models {
        model.created_ids = created_map.get(&model.id).cloned().unwrap_or_default();
    }

    Ok((StatusCode::OK, Json(models)))
}

/// Get hub assistants with locale support and created_ids for current user
#[debug_handler]
pub async fn get_hub_assistants(
    auth: RequirePermissions<(HubAssistantsRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubAssistantsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created assistant IDs for current user
    let created_map = Repos.hub.get_created_assistant_ids(auth.user.id).await?;

    // Merge created_ids into assistants
    let mut assistants = hub_data.assistants;
    for assistant in &mut assistants {
        assistant.created_ids = created_map.get(&assistant.id).cloned().unwrap_or_default();
    }

    Ok((StatusCode::OK, Json(assistants)))
}

/// Get hub MCP servers with locale support and created_ids for current user
#[debug_handler]
pub async fn get_hub_mcp_servers(
    auth: RequirePermissions<(HubMCPServersRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubMCPServersResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created MCP server IDs for current user
    let created_map = Repos.hub.get_created_mcp_server_ids(auth.user.id).await?;

    // Merge created_ids into servers
    let mut mcp_servers = hub_data.mcp_servers;
    for server in &mut mcp_servers {
        server.created_ids = created_map.get(&server.id).cloned().unwrap_or_default();
    }

    Ok((StatusCode::OK, Json(mcp_servers)))
}

/// Get hub models version
#[debug_handler]
pub async fn get_hub_models_version(
    _auth: RequirePermissions<(HubModelsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("llm-models").await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: None,
        }),
    ))
}

/// Get hub assistants version
#[debug_handler]
pub async fn get_hub_assistants_version(
    _auth: RequirePermissions<(HubAssistantsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("assistants").await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: None,
        }),
    ))
}

/// Get hub MCP servers version
#[debug_handler]
pub async fn get_hub_mcp_servers_version(
    _auth: RequirePermissions<(HubMCPServersVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("mcp-servers").await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: None,
        }),
    ))
}

/// Refresh hub models from GitHub
#[debug_handler]
pub async fn refresh_hub_models(
    _auth: RequirePermissions<(HubModelsRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("llm-models").await?;
    hub_manager.refresh_hub_category("llm-models").await?;
    let new_version = hub_manager.get_current_version("llm-models").await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::models_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

/// Refresh hub assistants from GitHub
#[debug_handler]
pub async fn refresh_hub_assistants(
    _auth: RequirePermissions<(HubAssistantsRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("assistants").await?;
    hub_manager.refresh_hub_category("assistants").await?;
    let new_version = hub_manager.get_current_version("assistants").await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::assistants_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

/// Refresh hub MCP servers from GitHub
#[debug_handler]
pub async fn refresh_hub_mcp_servers(
    _auth: RequirePermissions<(HubMCPServersRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("mcp-servers").await?;
    hub_manager.refresh_hub_category("mcp-servers").await?;
    let new_version = hub_manager.get_current_version("mcp-servers").await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::mcp_servers_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

// =====================================================
// ASSISTANT FROM HUB
// =====================================================

/// Create assistant from hub catalog
#[debug_handler]
pub async fn create_assistant_from_hub(
    auth: RequirePermissions<(HubAssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateAssistantFromHubRequest>,
) -> ApiResult<Json<AssistantFromHubResponse>> {
    // 1. Load hub assistant
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;

    let hub_assistant = hub_data
        .assistants
        .into_iter()
        .find(|a| a.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub assistant '{}'", request.hub_id)))?;

    // 2. Build create assistant request (WITHOUT source field)
    let create_request = crate::modules::assistant::types::CreateAssistantRequest {
        name: request.name.unwrap_or(hub_assistant.name.clone()),
        description: request.description.or(hub_assistant.description.clone()),
        instructions: request.instructions.or(hub_assistant.instructions.clone()),
        parameters: request
            .parameters
            .and_then(|p| serde_json::from_value::<ModelParameters>(p).ok())
            .or_else(|| {
                serde_json::from_value::<ModelParameters>(hub_assistant.parameters.clone()).ok()
            }),
        is_template: Some(false),
        is_default: Some(request.is_default),
        enabled: Some(request.enabled),
    };

    // 3. Create assistant via assistant module
    let assistant = Repos
        .assistant
        .create(Some(auth.user.id), create_request)
        .await?;

    // 4. Track in hub_entities
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::Assistant,
            assistant.id,
            &request.hub_id,
            HubCategory::Assistant,
            Some(auth.user.id),
        )
        .await?;

    // 5. Emit event
    event_bus.emit_async(
        HubEvent::assistant_created_from_hub(assistant.id, request.hub_id.clone()).into(),
    );

    // 6. Return combined response
    Ok((
        StatusCode::CREATED,
        Json(AssistantFromHubResponse {
            assistant,
            hub_tracking,
        }),
    ))
}

// =====================================================
// MCP SERVER FROM HUB
// =====================================================

/// Create MCP server from hub catalog
#[debug_handler]
pub async fn create_mcp_server_from_hub(
    auth: RequirePermissions<(HubMcpServersCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateMcpServerFromHubRequest>,
) -> ApiResult<Json<McpServerFromHubResponse>> {
    // 1. Load hub MCP server
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;

    let hub_server = hub_data
        .mcp_servers
        .into_iter()
        .find(|s| s.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub MCP server '{}'", request.hub_id)))?;

    // 2. Determine transport type
    let transport_type = hub_server
        .transport_type
        .as_ref()
        .and_then(|t| match t.as_str() {
            "stdio" => Some(crate::modules::mcp::TransportType::Stdio),
            "sse" => Some(crate::modules::mcp::TransportType::Sse),
            _ => None,
        })
        .unwrap_or(crate::modules::mcp::TransportType::Stdio);

    // 3. Build create MCP server request (WITHOUT source field)
    let create_request = crate::modules::mcp::CreateMcpServerRequest {
        name: request.name.unwrap_or(hub_server.name.clone()),
        display_name: request
            .display_name
            .unwrap_or(hub_server.display_name.clone()),
        description: hub_server.description.clone(),
        enabled: Some(request.enabled),
        transport_type,
        command: hub_server.command.clone(),
        args: hub_server.args.clone(),
        environment_variables: hub_server
            .environment_variables
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        url: hub_server.url.clone(),
        headers: hub_server
            .headers
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        timeout_seconds: Some(30),
    };

    // 4. Create user MCP server (hub interface only creates user servers, not system servers)
    let server = Repos
        .mcp
        .create_user_server(auth.user.id, create_request)
        .await?;

    // 5. Track in hub_entities
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::McpServer,
            server.id,
            &request.hub_id,
            HubCategory::McpServer,
            Some(auth.user.id),
        )
        .await?;

    // 6. Emit event
    event_bus.emit_async(
        HubEvent::mcp_server_created_from_hub(server.id, request.hub_id.clone()).into(),
    );

    // 7. Return combined response
    Ok((
        StatusCode::CREATED,
        Json(McpServerFromHubResponse {
            server,
            hub_tracking,
        }),
    ))
}

// =====================================================
// MODEL FROM HUB
// =====================================================

/// Create model download from hub catalog
#[debug_handler]
pub async fn create_model_from_hub(
    _auth: RequirePermissions<(HubModelsCreate,)>,

    Json(_request): Json<CreateModelFromHubRequest>,
) -> ApiResult<Json<ModelFromHubResponse>> {
    // Implementation for model download
    // TODO: Implement full model download logic
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        AppError::internal_error("Model download from hub not yet implemented"),
    ))
}

// =====================================================
// OpenAPI Documentation
// =====================================================

pub fn get_hub_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getModels")
        .tag("Hub")
        .summary("Get hub models with locale support")
        .response::<200, Json<HubModelsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsRead,)>(op)
        .id("Hub.getAssistants")
        .tag("Hub")
        .summary("Get hub assistants with locale support")
        .response::<200, Json<HubAssistantsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_mcp_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersRead,)>(op)
        .id("Hub.getMCPServers")
        .tag("Hub")
        .summary("Get hub MCP servers with locale support")
        .response::<200, Json<HubMCPServersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_models_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsVersionRead,)>(op)
        .id("Hub.getModelsVersion")
        .tag("Hub")
        .summary("Get hub models version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_assistants_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsVersionRead,)>(op)
        .id("Hub.getAssistantsVersion")
        .tag("Hub")
        .summary("Get hub assistants version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_mcp_servers_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersVersionRead,)>(op)
        .id("Hub.getMCPServersVersion")
        .tag("Hub")
        .summary("Get hub MCP servers version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn refresh_hub_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRefresh,)>(op)
        .id("Hub.refreshModels")
        .tag("Hub")
        .summary("Refresh hub models from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn refresh_hub_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsRefresh,)>(op)
        .id("Hub.refreshAssistants")
        .tag("Hub")
        .summary("Refresh hub assistants from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn refresh_hub_mcp_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersRefresh,)>(op)
        .id("Hub.refreshMCPServers")
        .tag("Hub")
        .summary("Refresh hub MCP servers from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn create_assistant_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsCreate,)>(op)
        .id("Hub.createAssistantFromHub")
        .tag("Hub")
        .summary("Create assistant from hub catalog")
        .response::<201, Json<AssistantFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub assistant not found"))
}

pub fn create_mcp_server_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMcpServersCreate,)>(op)
        .id("Hub.createMcpServerFromHub")
        .tag("Hub")
        .summary("Create MCP server from hub catalog")
        .response::<201, Json<McpServerFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub MCP server not found"))
}

pub fn create_model_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsCreate,)>(op)
        .id("Hub.createModelFromHub")
        .tag("Hub")
        .summary("Download model from hub catalog")
        .response::<201, Json<ModelFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub model not found"))
        .response_with::<501, (), _>(|res| res.description("Not yet implemented"))
}
