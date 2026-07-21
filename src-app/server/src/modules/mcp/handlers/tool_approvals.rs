//! Admin per-(server, tool) approval-mode defaults for MCP servers
//! (ITEM-54 / DEC-112).
//!
//! An admin views a SYSTEM MCP server's advertised tool list and sets an
//! approval-mode OVERRIDE per tool (`auto_approve` / `manual_approve` /
//! `disabled`). The chat approval gate (`chat_extension/mcp.rs`) consults these
//! overrides BEFORE the conversation / user default (override wins); an absent
//! entry leaves existing behavior unchanged.
//!
//! Storage is a jsonb map on the `mcp_servers` row (`tool_approval_defaults`) —
//! see `repository.rs`. These two endpoints are the ADMIN surface; the FE
//! tranche consumes them. Both gate on the same `mcp_servers_admin::*` perms
//! that gate the rest of the system-MCP-server admin surface.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        mcp::{
            chat_extension::ApprovalMode,
            client::{Tool, manager::McpSessionManager},
            permissions::{McpServersAdminEdit, McpServersAdminRead},
            tool_calls::models::McpToolCallSource,
        },
        permissions::{RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};
use axum::extract::Extension;

/// The fallback approval mode applied to a tool with no explicit admin override.
/// There is no per-server approval_mode column, so a tool without an override
/// simply follows the normal manual-approval flow (the conservative default).
const SERVER_DEFAULT_MODE: ApprovalMode = ApprovalMode::ManualApprove;

/// One advertised tool + its effective admin approval mode.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ToolApprovalEntry {
    /// Advertised tool name.
    pub tool_name: String,
    /// Advertised tool description (absent for override-only / unreachable tools).
    pub description: Option<String>,
    /// Effective admin mode = per-tool override ?? the server default.
    pub effective_mode: ApprovalMode,
    /// True when an explicit per-tool override is set (vs the server default).
    pub has_override: bool,
}

/// GET response: the server's advertised tools, each with its effective mode.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ServerToolApprovalsResponse {
    pub server_id: Uuid,
    /// Fallback applied to any tool without an explicit override.
    pub server_default_mode: ApprovalMode,
    /// True when the live `tools/list` probe failed — `tools` then lists only
    /// tools that already carry an override (the advertised set is unknown).
    pub tools_unreachable: bool,
    /// Human reason when `tools_unreachable`.
    pub unreachable_reason: Option<String>,
    /// Advertised tools (∪ any override-keyed tool not in the advertised set).
    pub tools: Vec<ToolApprovalEntry>,
}

/// PUT body: the override to set, or `null`/omitted to CLEAR it.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetToolApprovalRequest {
    /// `null` or omitted clears the override (the tool falls back to the server
    /// default). Otherwise sets the per-tool override to this mode.
    #[serde(default)]
    pub mode: Option<ApprovalMode>,
}

/// PUT response: the tool's effective mode after the change.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SetToolApprovalResponse {
    pub server_id: Uuid,
    pub tool_name: String,
    pub effective_mode: ApprovalMode,
    pub has_override: bool,
}

/// Resolve a tool's effective mode from the stored override map.
fn resolve_effective(overrides: &HashMap<String, String>, tool: &str) -> (ApprovalMode, bool) {
    match overrides.get(tool).and_then(|s| s.parse::<ApprovalMode>().ok()) {
        Some(m) => (m, true),
        None => (SERVER_DEFAULT_MODE, false),
    }
}

/// Live `tools/list` probe (same session path as `list_server_tools`).
async fn probe_tools(
    session_manager: &Arc<McpSessionManager>,
    server_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Tool>, AppError> {
    let session = session_manager
        .get_or_create_with_context(
            server_id,
            user_id,
            None,
            None,
            None,
            None,
            McpToolCallSource::Rest,
        )
        .await?;
    let mut session = session.write().await;
    session.list_tools().await
}

/// GET /api/mcp/servers/{id}/tool-approvals — advertised tools + effective modes
/// for a SYSTEM MCP server.
#[debug_handler]
pub async fn get_server_tool_approvals(
    auth: RequirePermissions<(McpServersAdminRead,)>,
    Extension(session_manager): Extension<Arc<McpSessionManager>>,
    Path(server_id): Path<Uuid>,
) -> ApiResult<Json<ServerToolApprovalsResponse>> {
    // Existence + is_system gate (also returns the stored overrides): a foreign
    // / user-owned / missing id → 404.
    let overrides = Repos
        .mcp
        .get_system_server_tool_approvals(server_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Live tools/list; on failure, fall back to the override-keyed tools only so
    // the admin can still see + clear existing overrides on an unreachable server.
    let (advertised, tools_unreachable, unreachable_reason) =
        match probe_tools(&session_manager, server_id, auth.user.id).await {
            Ok(tools) => (tools, false, None),
            Err(e) => (Vec::new(), true, Some(e.to_string())),
        };

    let mut seen: HashSet<String> = HashSet::new();
    let mut tools: Vec<ToolApprovalEntry> = Vec::new();
    for t in advertised {
        seen.insert(t.name.clone());
        let (effective_mode, has_override) = resolve_effective(&overrides, &t.name);
        tools.push(ToolApprovalEntry {
            tool_name: t.name,
            description: t.description,
            effective_mode,
            has_override,
        });
    }
    // Include override-keyed tools not in the advertised list (stale entries, or
    // everything when unreachable) so the admin can still see + clear them.
    let mut extra: Vec<String> = overrides
        .keys()
        .filter(|k| !seen.contains(*k))
        .cloned()
        .collect();
    extra.sort();
    for name in extra {
        let (effective_mode, has_override) = resolve_effective(&overrides, &name);
        tools.push(ToolApprovalEntry {
            tool_name: name,
            description: None,
            effective_mode,
            has_override,
        });
    }

    Ok((
        StatusCode::OK,
        Json(ServerToolApprovalsResponse {
            server_id,
            server_default_mode: SERVER_DEFAULT_MODE,
            tools_unreachable,
            unreachable_reason,
            tools,
        }),
    ))
}

pub fn get_server_tool_approvals_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminRead,)>(op)
        .id("McpServerToolApprovals.get")
        .tag("MCP Servers - System")
        .summary("List a system MCP server's tools + per-tool approval modes")
        .description(
            "Return a SYSTEM MCP server's advertised tools (live `tools/list`, \
             with an `tools_unreachable` fallback when the server can't be \
             reached) and, per tool, the EFFECTIVE admin approval mode = the \
             per-tool override if set, else the server default (`manual_approve`). \
             Admin-only (`mcp_servers_admin::read`). A foreign / non-system id \
             returns 404.",
        )
        .response::<200, Json<ServerToolApprovalsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Forbidden"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}

/// PUT /api/mcp/servers/{id}/tool-approvals/{tool} — set/clear one tool's
/// admin approval override on a SYSTEM MCP server.
#[debug_handler]
pub async fn set_server_tool_approval(
    auth: RequirePermissions<(McpServersAdminEdit,)>,
    origin: SyncOrigin,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    Json(request): Json<SetToolApprovalRequest>,
) -> ApiResult<Json<SetToolApprovalResponse>> {
    // Bind is the authz gate; no other use of the user here.
    let _ = &auth;

    let updated = Repos
        .mcp
        .set_system_server_tool_approval(server_id, &tool_name, request.mode.clone())
        .await?;
    if !updated {
        // Absent / not a system server → 404 (never leak existence).
        return Err(AppError::not_found("Server").into());
    }

    // Reuse the existing system-server sync entity — no new SyncEntity variant
    // needed. The FE refetches the server (+ its tool-approvals) on this signal.
    sync_publish(
        SyncEntity::McpServerSystem,
        SyncAction::Update,
        server_id,
        Audience::perm::<McpServersAdminRead>(),
        origin.0,
    );

    let (effective_mode, has_override) = match &request.mode {
        Some(m) => (m.clone(), true),
        None => (SERVER_DEFAULT_MODE, false),
    };
    Ok((
        StatusCode::OK,
        Json(SetToolApprovalResponse {
            server_id,
            tool_name,
            effective_mode,
            has_override,
        }),
    ))
}

pub fn set_server_tool_approval_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminEdit,)>(op)
        .id("McpServerToolApprovals.set")
        .tag("MCP Servers - System")
        .summary("Set or clear a system MCP server tool's approval override")
        .description(
            "Set (`mode`) or clear (`mode: null`) the ADMIN per-tool approval \
             override for one tool of a SYSTEM MCP server. The override is \
             consulted by the chat approval gate BEFORE the conversation / user \
             default (override wins). Admin-only (`mcp_servers_admin::edit`). A \
             foreign / non-system id returns 404.",
        )
        .response::<200, Json<SetToolApprovalResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Forbidden"))
        .response_with::<404, (), _>(|res| res.description("Server not found"))
}
