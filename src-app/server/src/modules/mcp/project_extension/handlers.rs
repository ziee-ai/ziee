// HTTP handlers for `/api/projects/{id}/mcp-settings` (GET + PUT).
// Relocated from `modules/project/handlers.rs` as part of the project↔mcp
// inversion (migration 78).

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Path,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::mcp::chat_extension::approval::models::AutoApprovedServer;
use crate::modules::mcp::chat_extension::defaults::models::LoopSettings;
use crate::modules::mcp::settings::{McpScope, McpSettings};
use crate::modules::permissions::{extractors::RequirePermissions, with_permission};
use crate::modules::project::permissions::{ProjectsEdit, ProjectsRead};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::models::ProjectMcpSettingsRequest;

/// GET /api/projects/{id}/mcp-settings — return the project's MCP
/// settings (or defaults if no row exists).
#[debug_handler]
pub async fn get_project_mcp_settings(
    auth: RequirePermissions<(ProjectsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ProjectMcpSettingsResponse>> {
    // Ownership check via project repo (project still owns the projects
    // row; this is the file → project import direction, allowed).
    let _project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    let settings = Repos
        .mcp_settings
        .get_or_default(McpScope::Project(id), auth.user.id)
        .await?;
    Ok((StatusCode::OK, Json(ProjectMcpSettingsResponse::from(settings))))
}

pub fn get_project_mcp_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.getMcpSettings")
        .tag("Projects")
        .summary("Get project MCP defaults")
        .description(
            "Read the project's MCP defaults (approval mode + auto-approved tools \
             + disabled servers + loop settings). These apply to NEW conversations \
             created in the project — existing conversations keep their own \
             snapshot taken at attach time.",
        )
        .response::<200, Json<ProjectMcpSettingsResponse>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
}

/// PUT /api/projects/{id}/mcp-settings — upsert the project's MCP
/// settings. Validates that every referenced server_id is accessible
/// to the user before writing.
#[debug_handler]
pub async fn update_project_mcp_settings(
    auth: RequirePermissions<(ProjectsEdit,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
    Json(request): Json<ProjectMcpSettingsRequest>,
) -> ApiResult<Json<ProjectMcpSettingsResponse>> {
    let project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    // Validate user has access to every referenced MCP server. Closes
    // Round-4 boundary audit (project↔mcp #3): without this, a client
    // could PUT arbitrary UUIDs and every conversation snapshotted
    // from this project would carry dangling MCP references that
    // silently fail at chat-send time.
    let auto_approved_ids: Vec<Uuid> = request
        .auto_approved_tools
        .iter()
        .map(|s| s.server_id)
        .collect();
    let disabled_ids: Vec<Uuid> = request
        .disabled_servers
        .iter()
        .map(|s| s.server_id)
        .collect();
    validate_mcp_server_access(auth.user.id, auto_approved_ids, "auto_approved_tools").await?;
    validate_mcp_server_access(auth.user.id, disabled_ids, "disabled_servers").await?;

    let auto_approved_json = serde_json::to_value(&request.auto_approved_tools)
        .map_err(|e| AppError::internal_error(format!("serialize auto_approved_tools: {e}")))?;
    let disabled_json = serde_json::to_value(&request.disabled_servers)
        .map_err(|e| AppError::internal_error(format!("serialize disabled_servers: {e}")))?;
    let loop_json = match &request.loop_settings {
        Some(ls) => Some(
            serde_json::to_value(ls)
                .map_err(|e| AppError::internal_error(format!("serialize loop_settings: {e}")))?,
        ),
        None => None,
    };

    let saved = Repos
        .mcp_settings
        .upsert(
            McpScope::Project(project.id),
            auth.user.id,
            crate::modules::mcp::settings::models::McpSettingsUpdate {
                approval_mode: Some(request.approval_mode.to_string()),
                auto_approved_tools: Some(auto_approved_json),
                disabled_servers: Some(disabled_json),
                loop_settings: Some(loop_json),
            },
        )
        .await?;

    // Notify the owner's other devices so an open project-detail MCP settings
    // page refetches (notify-only; the settings surface is the only affected
    // view — existing conversations keep their attach-time snapshot).
    sync_publish(
        SyncEntity::Project,
        SyncAction::Update,
        project.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::OK, Json(ProjectMcpSettingsResponse::from(saved))))
}

pub fn update_project_mcp_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsEdit,)>(op)
        .id("Project.updateMcpSettings")
        .tag("Projects")
        .summary("Update project MCP defaults")
        .description(
            "Upsert the project's MCP defaults. These apply to NEW conversations \
             created in the project (snapshot at attach time); existing \
             conversations are not affected.\n\
             \n\
             Validation:\n\
             - Every server_id in `auto_approved_tools` and `disabled_servers` \
               must reference an MCP server the calling user can access.",
        )
        .response::<200, Json<ProjectMcpSettingsResponse>>()
        .response_with::<400, (), _>(|res| res.description("Validation error"))
        .response_with::<404, (), _>(|res| res.description("Project not found"))
        .response_with::<422, (), _>(|res| res.description("MCP server not accessible"))
}

/// GET response shape. Wraps an `McpSettings` row in the same wire
/// format as the legacy `UpdateProjectMcpSettingsRequest` so the
/// frontend's autogen client stays compatible.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectMcpSettingsResponse {
    pub approval_mode: String,
    pub auto_approved_tools: Vec<AutoApprovedServer>,
    pub disabled_servers: Vec<crate::modules::mcp::chat_extension::approval::models::DisabledServer>,
    pub loop_settings: Option<LoopSettings>,
}

impl From<McpSettings> for ProjectMcpSettingsResponse {
    fn from(settings: McpSettings) -> Self {
        let auto_approved_tools = serde_json::from_value(settings.auto_approved_tools.clone())
            .unwrap_or_default();
        let disabled_servers = serde_json::from_value(settings.disabled_servers.clone())
            .unwrap_or_default();
        let loop_settings = settings
            .loop_settings
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        Self {
            approval_mode: settings.approval_mode,
            auto_approved_tools,
            disabled_servers,
            loop_settings,
        }
    }
}

/// Verify every server_id is accessible to the user. Single-shot —
/// returns the first dangling reference as a 422 with a clear code.
async fn validate_mcp_server_access<I: IntoIterator<Item = Uuid>>(
    user_id: Uuid,
    server_ids: I,
    field: &str,
) -> Result<(), AppError> {
    for server_id in server_ids {
        let accessible = Repos
            .mcp
            .can_user_access_server(user_id, server_id)
            .await?;
        if !accessible {
            return Err(AppError::unprocessable_entity(
                "MCP_SERVER_NOT_ACCESSIBLE",
                format!(
                    "{field} references MCP server {server_id} which you don't have access to"
                ),
            ));
        }
    }
    Ok(())
}
