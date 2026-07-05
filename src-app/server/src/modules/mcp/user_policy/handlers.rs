//! REST handlers for /api/mcp/user-policy.

use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, http::StatusCode};
use std::sync::Arc;

use crate::common::ApiResult;
use crate::core::{EventBus, Repos};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::super::events::McpServerEvent;
use super::super::permissions::{McpServersRead, McpUserPolicyEdit};
use super::repository;
use super::types::{McpUserPolicy, UpdateMcpUserPolicyRequest};

// =====================================================
// GET /api/mcp/user-policy  (any user with mcp_servers::read)
// =====================================================
//
// Gated on `McpServersRead` rather than "any authenticated user".
// Every user who can interact with MCP at all already has this perm,
// so it's the right scope: a user who cannot read MCP servers has no
// use for the policy that governs them.

#[debug_handler]
pub async fn get_user_policy(
    _auth: RequirePermissions<(McpServersRead,)>,
) -> ApiResult<Json<McpUserPolicy>> {
    let policy = repository::load(Repos.pool()).await?;
    Ok((StatusCode::OK, Json(policy)))
}

pub fn get_user_policy_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("McpUserPolicy.get")
        .tag("MCP Servers - User Policy")
        .summary("Get MCP user policy")
        .description(
            "Read the global MCP user policy. Gated on `mcp_servers::read` — \
             the UI uses this to gate the Add button on /settings/mcp-servers \
             and the visibility of the MCP tab in the Hub for users who can \
             read MCP servers but don't have admin install rights.",
        )
        .response::<200, Json<McpUserPolicy>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// =====================================================
// PUT /api/mcp/user-policy  (admin only)
// =====================================================

#[debug_handler]
pub async fn update_user_policy(
    auth: RequirePermissions<(McpUserPolicyEdit,)>,
    origin: SyncOrigin,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<UpdateMcpUserPolicyRequest>,
) -> ApiResult<Json<McpUserPolicy>> {
    let policy = repository::save(Repos.pool(), auth.user.id, request).await?;

    // Emit a server-side event so future audit-log subscribers can
    // record the change. The frontend emits its own
    // `mcp_user_policy.changed` event on the FE event bus from the
    // store after the PUT resolves — those two are separate
    // mechanisms (this is for backend observers, the FE one is for
    // in-browser reactivity).
    event_bus.emit_async(McpServerEvent::user_policy_updated(
        auth.user.id,
        policy.allowed_transports.clone(),
        policy.user_stdio_sandbox_flavor.clone(),
    ));

    // The MCP user policy is a deployment-wide singleton read by every user
    // holding `mcp_servers::read`; notify them so their devices refetch the
    // (sanitized) policy via `GET /api/mcp/user-policy`.
    sync_publish(
        SyncEntity::McpUserPolicy,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<McpServersRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(policy)))
}

pub fn update_user_policy_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpUserPolicyEdit,)>(op)
        .id("McpUserPolicy.update")
        .tag("MCP Servers - User Policy")
        .summary("Update MCP user policy")
        .description(
            "Update the global MCP user policy: which transports regular \
             users may install (subset of {\"http\",\"stdio\"}) and, when \
             stdio is allowed, which sandbox flavor user-installed stdio \
             servers must run inside. 422 on validation errors \
             (MCP_INVALID_TRANSPORT, MCP_FLAVOR_REQUIRED, MCP_UNKNOWN_FLAVOR, \
             MCP_SANDBOX_DISABLED).",
        )
        .response::<200, Json<McpUserPolicy>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        // 403 is set by `with_permission` above (with the required
        // permission name embedded so the FE codegen extracts
        // `McpUserPolicyEdit` into `Permissions`). Don't override.
        .response_with::<422, (), _>(|res| res.description("Validation failed"))
}
