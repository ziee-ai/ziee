use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, extract::Query, http::StatusCode};

use crate::{
    common::{ApiResult, AppError},
    core::events::EventBus,
    modules::{
        llm_model::{ModelParameters, permissions::LlmModelsCreate},
        permissions::{RequirePermissions, with_permission},
    },
};
use std::sync::Arc;

use super::{
    events::HubEvent,
    hub_manager::{Catalog, HubManager, HubManifest},
    models::{HubCategory, HubEntityType},
    permissions::*,
    types::*,
};
use axum::extract::Path as AxumPath;

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
            "http" => Some(crate::modules::mcp::TransportType::Http),
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
        timeout_seconds: Some(if hub_server.supports_sampling == Some(true) { 300 } else { 30 }),
        supports_sampling: hub_server.supports_sampling,
        usage_mode: None,
        max_concurrent_sessions: None,
        // Hub installs are user-scoped (line 369 below), and the
        // sandbox option only honors admin/system servers — always
        // off for hub-installed servers.
        run_in_sandbox: None,
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

/// Create model download from hub catalog.
///
/// SECURITY: requires BOTH hub::models::create AND llm_models::create.
/// The handler routes to `initiate_repository_download_internal` which
/// bypasses the llm_models::create permission check that the
/// equivalent /llm-models/download endpoint applies — so without the
/// added LlmModelsCreate requirement here, anyone with just
/// hub::models::create could create models via this back-door. Closes
/// 11-hub F-05 (Medium).
#[debug_handler]
pub async fn create_model_from_hub(
    _auth: RequirePermissions<(HubModelsCreate, LlmModelsCreate)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateModelFromHubRequest>,
) -> ApiResult<Json<ModelFromHubResponse>> {
    use crate::modules::llm_model::models::FileFormat as LlmFileFormat;

    // 1. Load hub model
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;

    let hub_model = hub_data
        .models
        .into_iter()
        .find(|m| m.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub model '{}'", request.hub_id)))?;

    // 2. Find repository by URL
    let repository = Repos
        .llm_repository
        .find_by_url(&hub_model.repository_url)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(&format!(
                "Repository with URL '{}' not found",
                hub_model.repository_url
            ))
        })?;

    // 3. Select quantization option if specified
    let main_filename = if let Some(ref quant_name) = request.quantization_name {
        hub_model
            .quantization_options
            .as_ref()
            .and_then(|opts| {
                opts.iter()
                    .find(|o| &o.name == quant_name)
                    .map(|opt| opt.main_filename.clone())
            })
            .unwrap_or_else(|| hub_model.main_filename.clone())
    } else {
        hub_model.main_filename.clone()
    };

    // 4. Convert FileFormat from hub to llm_model
    let file_format = match hub_model.file_format {
        super::models::FileFormat::GGUF => LlmFileFormat::Gguf,
        super::models::FileFormat::SafeTensors => LlmFileFormat::Safetensors,
        super::models::FileFormat::PyTorch => LlmFileFormat::Pytorch,
    };

    // 5. Convert capabilities from hub to llm_model format
    let capabilities = hub_model.capabilities.map(|hub_caps| {
        crate::modules::llm_model::models::ModelCapabilities {
            vision: Some(hub_caps.vision),
            audio: Some(hub_caps.audio),
            tools: Some(hub_caps.tools),
            code_interpreter: Some(hub_caps.code_interpreter),
            chat: Some(hub_caps.chat),
            text_embedding: Some(hub_caps.text_embedding),
            image_generator: Some(hub_caps.image_generator),
        }
    });

    // 6. Build download request for initiate_repository_download
    let download_request = crate::modules::llm_model::handlers::uploads::DownloadFromRepositoryRequest {
        provider_id: request.provider_id,
        repository_id: repository.id,
        repository_path: hub_model.repository_path.clone(),
        repository_branch: None,
        name: hub_model.name.clone(),
        display_name: request
            .display_name
            .unwrap_or_else(|| hub_model.display_name.clone()),
        description: hub_model.description.clone(),
        file_format,
        main_filename,
        capabilities,
        parameters: hub_model
            .recommended_parameters
            .and_then(|p| serde_json::from_value(p).ok()),
        engine_type: hub_model
            .recommended_engine
            .and_then(|e| crate::modules::llm_model::models::EngineType::from_str(&e)),
        engine_settings: hub_model
            .recommended_engine_settings
            .and_then(|s| serde_json::from_value(s).ok()),
    };

    // 7. Initiate the actual download (this creates the download instance AND spawns the background task)
    let download = crate::modules::llm_model::handlers::uploads::initiate_repository_download_internal(
        download_request,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Failed to initiate download: {}", e)),
        )
    })?;

    // 8. Track in hub_entities
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::LlmModel,
            download.id,
            &request.hub_id,
            HubCategory::Model,
            None, // Models are system-wide, not user-specific
        )
        .await?;

    // 9. Emit event
    event_bus.emit_async(
        HubEvent::model_download_started_from_hub(download.id, request.hub_id.clone()).into(),
    );

    // 10. Return response
    Ok((
        StatusCode::CREATED,
        Json(ModelFromHubResponse {
            download,
            hub_tracking,
        }),
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
    with_permission::<(HubModelsCreate, LlmModelsCreate)>(op)
        .id("Hub.createModelFromHub")
        .tag("Hub")
        .summary("Download model from hub catalog")
        .response::<201, Json<ModelFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub model not found"))
        .response_with::<501, (), _>(|res| res.description("Not yet implemented"))
}

// =====================================================
// LOCAL PROVIDERS FOR HUB DOWNLOADS
// =====================================================

/// List enabled local providers available as download targets for hub models
#[debug_handler]
pub async fn get_hub_local_providers(
    _auth: RequirePermissions<(HubModelsCreate,)>,
) -> ApiResult<Json<HubLocalProvidersResponse>> {
    let providers = Repos.llm_provider.list_local_providers().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Database error: {}", e)),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(HubLocalProvidersResponse {
            providers: providers
                .into_iter()
                .map(|p| HubLocalProvider { id: p.id, name: p.name })
                .collect(),
        }),
    ))
}

pub fn get_hub_local_providers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsCreate,)>(op)
        .id("Hub.getLocalProviders")
        .tag("Hub")
        .summary("List local providers available for hub model downloads")
        .response::<200, Json<HubLocalProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// =====================================================
// UNIFIED CATALOG ENDPOINTS (new in Phase 1)
// =====================================================

/// GET /api/hub/index — return the full parsed catalog (flat across
/// all categories). Cheap: reads ~6 KB of JSON. The Phase-2 frontend
/// will load this once per session and client-side-filter into the
/// existing three tabs.
#[debug_handler]
pub async fn get_hub_catalog(
    _auth: RequirePermissions<(HubModelsRead,)>,
) -> ApiResult<Json<Catalog>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    Ok((StatusCode::OK, Json(catalog)))
}

pub fn get_hub_catalog_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getCatalog")
        .tag("Hub")
        .summary("Get the unified hub catalog (index.json)")
        .response::<200, Json<Catalog>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/hub/version — catalog hub_version, the running server's
/// own semver (for client-side compat filtering), per-category counts.
#[debug_handler]
pub async fn get_hub_catalog_version(
    _auth: RequirePermissions<(HubModelsRead,)>,
) -> ApiResult<Json<HubCatalogVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    let mut counts = HubCatalogCounts {
        models: 0,
        assistants: 0,
        mcp_servers: 0,
    };
    for item in &catalog.items {
        match item.category {
            HubCategory::Model => counts.models += 1,
            HubCategory::Assistant => counts.assistants += 1,
            HubCategory::McpServer => counts.mcp_servers += 1,
        }
    }
    Ok((
        StatusCode::OK,
        Json(HubCatalogVersionResponse {
            hub_version: catalog.hub_version,
            server_version: super::hub_manager::server_version().to_string(),
            counts,
        }),
    ))
}

pub fn get_hub_catalog_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getCatalogVersion")
        .tag("Hub")
        .summary("Current hub catalog version + server version + counts")
        .response::<200, Json<HubCatalogVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// POST /api/hub/refresh — admin-only force fetch from GitHub.
/// Cosign + sha256 failure leaves the previous catalog in place.
#[debug_handler]
pub async fn refresh_hub_catalog(
    _auth: RequirePermissions<(HubAdmin,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubCatalogRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let outcome = hub_manager.refresh().await?;

    if outcome.updated {
        // Reuse the existing per-category events so any listener wired
        // to one of them still picks up the change. The new catalog is
        // unified — three identical events emit at once.
        let prev = outcome.previous_version.clone().unwrap_or_default();
        event_bus.emit_async(
            HubEvent::models_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::assistants_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::mcp_servers_refreshed(prev, outcome.new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubCatalogRefreshResponse {
            updated: outcome.updated,
            previous_version: outcome.previous_version,
            new_version: outcome.new_version,
            cosign_verified: outcome.cosign_verified,
        }),
    ))
}

pub fn refresh_hub_catalog_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAdmin,)>(op)
        .id("Hub.refreshCatalog")
        .tag("Hub")
        .summary("Force-refresh the hub catalog from GitHub Releases (admin only)")
        .response::<200, Json<HubCatalogRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| {
            res.description("Fetch / sha256 / cosign verify failure — previous catalog left in place")
        })
}

/// GET /api/hub/updates — admin-only. Installed entities whose
/// `hub_version` lags the catalog. NULL `hub_version` (legacy rows)
/// counts as behind.
#[debug_handler]
pub async fn get_hub_updates(
    _auth: RequirePermissions<(HubAdmin,)>,
) -> ApiResult<Json<HubUpdatesResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    let rows = Repos
        .hub
        .list_outdated_entities(&catalog.hub_version)
        .await?;
    Ok((
        StatusCode::OK,
        Json(HubUpdatesResponse {
            catalog_version: catalog.hub_version.clone(),
            updates: rows
                .into_iter()
                .map(|r| HubUpdateRow {
                    hub_id: r.hub_id,
                    hub_category: r.hub_category,
                    entity_type: r.entity_type,
                    entity_id: r.entity_id,
                    installed_version: r.installed_version,
                    current_version: catalog.hub_version.clone(),
                })
                .collect(),
        }),
    ))
}

pub fn get_hub_updates_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAdmin,)>(op)
        .id("Hub.getUpdates")
        .tag("Hub")
        .summary("Installed hub entities behind the current catalog version (admin only)")
        .response::<200, Json<HubUpdatesResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/hub/manifest/:id?category=... — full YAML manifest for one
/// item. Backs the detail-drawer view in the Phase-2 frontend so the
/// list view can stay small (just the index entries).
#[debug_handler]
pub async fn get_hub_manifest(
    _auth: RequirePermissions<(HubModelsRead,)>,
    AxumPath(id): AxumPath<String>,
    Query(q): Query<HubManifestQuery>,
) -> ApiResult<Json<HubManifest>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let manifest = hub_manager.manifest(q.category, &id).await?;
    Ok((StatusCode::OK, Json(manifest)))
}

pub fn get_hub_manifest_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getManifest")
        .tag("Hub")
        .summary("Full manifest for one hub item (model / assistant / mcp-server)")
        .response::<200, Json<HubManifest>>()
        .response_with::<400, (), _>(|res| res.description("Invalid id"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Manifest not found in catalog"))
}
