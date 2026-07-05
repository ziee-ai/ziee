//! User MCP defaults handlers

use aide::transform::TransformOperation;
use axum::{debug_handler, http::StatusCode, Json};
use serde::Serialize;

use crate::{
    common::ApiResult,
    modules::{
        chat::core::permissions::*,
        permissions::{extractors::RequirePermissions, with_permission},
        sync::{publish as sync_publish, Audience, SyncAction, SyncEntity, SyncOrigin},
    },
};

use super::{models, repository};

// =====================================================
// Request/Response Types
// =====================================================

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct UserMcpDefaultsGetResponse {
    pub defaults: Option<models::UserMcpDefaultsResponse>,
}

// =====================================================
// Handlers
// =====================================================

/// Get MCP defaults for the current user
#[debug_handler]
pub async fn get_mcp_defaults(
    auth: RequirePermissions<(ConversationsRead,)>,
) -> ApiResult<Json<UserMcpDefaultsGetResponse>> {
    let defaults = repository::get_user_defaults(crate::core::Repos.pool(), auth.user.id)
        .await?
        .map(models::UserMcpDefaultsResponse::from);

    Ok((StatusCode::OK, Json(UserMcpDefaultsGetResponse { defaults })))
}

pub fn get_mcp_defaults_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Mcp.getDefaults")
        .tag("MCP")
        .summary("Get user MCP defaults")
        .description("Get the current user's default MCP settings for new conversations")
        .response::<200, Json<UserMcpDefaultsGetResponse>>()
}

/// Update MCP defaults for the current user
#[debug_handler]
pub async fn update_mcp_defaults(
    auth: RequirePermissions<(ConversationsEdit,)>,
    origin: SyncOrigin,
    Json(request): Json<models::UpsertUserMcpDefaultsRequest>,
) -> ApiResult<Json<models::UserMcpDefaultsResponse>> {
    let defaults = repository::upsert_user_defaults(
        crate::core::Repos.pool(),
        auth.user.id,
        request.approval_mode,
        request.auto_approved_tools.as_deref(),
        &request.disabled_servers,
        &request.loop_settings,
    )
    .await?;

    // The user's default MCP settings are a per-user singleton; notify the
    // owner's other devices so they refetch `GET /api/mcp/defaults`.
    sync_publish(
        SyncEntity::McpDefaults,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((
        StatusCode::OK,
        Json(models::UserMcpDefaultsResponse::from(defaults)),
    ))
}

pub fn update_mcp_defaults_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Mcp.updateDefaults")
        .tag("MCP")
        .summary("Update user MCP defaults")
        .description("Create or update the current user's default MCP settings for new conversations")
        .response::<200, Json<models::UserMcpDefaultsResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
}
