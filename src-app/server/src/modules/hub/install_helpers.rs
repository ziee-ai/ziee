//! Orchestration helpers that combine a hub-catalog version lookup
//! (`HubManager::current_version`) with a `hub_entities` tracking
//! insert (`Repos.hub.track_hub_entity`). Called by the regular MCP
//! create handlers when the drawer-prefilled flow includes a
//! `hub_id` in the create request body — keeps the "already
//! installed" badge on the Hub MCP card working without having to
//! route through the dedicated `/hub/mcp-servers/create*` endpoints.
//!
//! Lives outside `repository.rs` because the lookup needs the hub
//! manager (filesystem + catalog), not just the DB pool — and
//! outside `handlers.rs` because they're not HTTP handlers, just
//! cross-module bookkeeping.

use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;

use super::hub_manager::HubManager;
use super::models::{HubCategory, HubEntityType};

/// Stamp a freshly-created USER MCP server as a hub install. The
/// regular `POST /api/mcp/servers` handler calls this when
/// `CreateMcpServerRequest.hub_id` is set (drawer opened from the
/// Hub MCP card's "Install for me" button). Mirrors the bookkeeping
/// the dedicated `/hub/mcp-servers/create` endpoint does so the
/// "already installed" badge keeps working.
pub async fn track_user_mcp_install(
    server_id: Uuid,
    hub_id: &str,
    user_id: Uuid,
) -> Result<(), AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_version = hub_manager.current_version().await.ok();
    Repos
        .hub
        .track_hub_entity(
            HubEntityType::McpServer,
            server_id,
            hub_id,
            HubCategory::McpServer,
            Some(user_id),
            hub_version.as_deref(),
        )
        .await?;
    Ok(())
}

/// SYSTEM analog of [`track_user_mcp_install`]. Called from
/// `POST /api/mcp/system-servers` when the request carries `hub_id`
/// (drawer opened from "Install for the system").
pub async fn track_system_mcp_install(
    server_id: Uuid,
    hub_id: &str,
) -> Result<(), AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_version = hub_manager.current_version().await.ok();
    Repos
        .hub
        .track_hub_entity(
            HubEntityType::McpServer,
            server_id,
            hub_id,
            HubCategory::McpServer,
            None,
            hub_version.as_deref(),
        )
        .await?;
    Ok(())
}
