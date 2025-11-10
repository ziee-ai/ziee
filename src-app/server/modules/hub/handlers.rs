use aide::transform::TransformOperation;
use axum::{
    extract::Query,
    http::StatusCode,
    Json,
};

use crate::{
    common::{ApiResult, AppError},
    modules::permissions::{RequirePermissions, with_permission},
};

use super::{
    permissions::*,
    types::*,
    hub_manager::HubManager,
};

// =====================================================
// Route Handlers
// =====================================================

/// Get hub models with locale support
pub async fn get_hub_models(
    _auth: RequirePermissions<(HubModelsRead,)>,
    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubModelsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    Ok((StatusCode::OK, Json(hub_data.models)))
}

/// Get hub assistants with locale support
pub async fn get_hub_assistants(
    _auth: RequirePermissions<(HubAssistantsRead,)>,
    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubAssistantsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    Ok((StatusCode::OK, Json(hub_data.assistants)))
}

/// Get hub MCP servers with locale support
pub async fn get_hub_mcp_servers(
    _auth: RequirePermissions<(HubMCPServersRead,)>,
    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubMCPServersResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    Ok((StatusCode::OK, Json(hub_data.mcp_servers)))
}

/// Get hub models version
pub async fn get_hub_models_version(
    _auth: RequirePermissions<(HubModelsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("llm-models").await?;

    Ok((StatusCode::OK, Json(HubVersionResponse {
        version,
        last_updated: None,
    })))
}

/// Get hub assistants version
pub async fn get_hub_assistants_version(
    _auth: RequirePermissions<(HubAssistantsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("assistants").await?;

    Ok((StatusCode::OK, Json(HubVersionResponse {
        version,
        last_updated: None,
    })))
}

/// Get hub MCP servers version
pub async fn get_hub_mcp_servers_version(
    _auth: RequirePermissions<(HubMCPServersVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.get_current_version("mcp-servers").await?;

    Ok((StatusCode::OK, Json(HubVersionResponse {
        version,
        last_updated: None,
    })))
}

/// Refresh hub models from GitHub
pub async fn refresh_hub_models(
    _auth: RequirePermissions<(HubModelsRefresh,)>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("llm-models").await?;
    hub_manager.refresh_hub_category("llm-models").await?;
    let new_version = hub_manager.get_current_version("llm-models").await?;

    Ok((StatusCode::OK, Json(HubRefreshResponse {
        updated: old_version != new_version,
        version: new_version,
    })))
}

/// Refresh hub assistants from GitHub
pub async fn refresh_hub_assistants(
    _auth: RequirePermissions<(HubAssistantsRefresh,)>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("assistants").await?;
    hub_manager.refresh_hub_category("assistants").await?;
    let new_version = hub_manager.get_current_version("assistants").await?;

    Ok((StatusCode::OK, Json(HubRefreshResponse {
        updated: old_version != new_version,
        version: new_version,
    })))
}

/// Refresh hub MCP servers from GitHub
pub async fn refresh_hub_mcp_servers(
    _auth: RequirePermissions<(HubMCPServersRefresh,)>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.get_current_version("mcp-servers").await?;
    hub_manager.refresh_hub_category("mcp-servers").await?;
    let new_version = hub_manager.get_current_version("mcp-servers").await?;

    Ok((StatusCode::OK, Json(HubRefreshResponse {
        updated: old_version != new_version,
        version: new_version,
    })))
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
