//! REST handlers for the agent module.
//!
//!   `GET/PUT /api/agent/settings` — deployment-wide agent policy singleton.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};

use crate::{
    common::ApiResult,
    core::Repos,
    modules::{
        agent::{
            models::{AgentAdminSettings, UpdateAgentAdminSettingsRequest},
            permissions::{AgentSettingsManage, AgentSettingsRead},
        },
        permissions::{RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};

#[debug_handler]
pub async fn get_admin_settings(
    _auth: RequirePermissions<(AgentSettingsRead,)>,
) -> ApiResult<Json<AgentAdminSettings>> {
    let row = Repos.agent.get_admin_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AgentSettingsRead,)>(op)
        .id("AgentAdmin.get")
        .tag("Agent")
        .summary("Read the deployment-wide agent policy")
        .description(
            "Returns the singleton deployment-wide agent policy (sandbox/approval \
             mode, reviewer config, token caps, max steps, fan-out guardrails).",
        )
        .response::<200, Json<AgentAdminSettings>>()
}

#[debug_handler]
pub async fn update_admin_settings(
    _auth: RequirePermissions<(AgentSettingsManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateAgentAdminSettingsRequest>,
) -> ApiResult<Json<AgentAdminSettings>> {
    // Bounds + enum validation up front so a bad value is a 400, not a raw
    // 500 from the DB CHECK.
    body.validate()?;

    let row = Repos.agent.update_admin_settings(&body).await?;

    // Notify-only: the sync payload carries just `{id}` (Uuid::nil for a
    // singleton) — the admin UI refetches via GET.
    sync_publish(
        SyncEntity::AgentAdminSettings,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<AgentSettingsRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(row)))
}

pub fn update_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AgentSettingsManage,)>(op)
        .id("AgentAdmin.update")
        .tag("Agent")
        .summary("Update the deployment-wide agent policy")
        .description(
            "Tri-state partial update — every field is optional. Missing field = \
             no change; explicit JSON null clears `reviewer_model_id` / \
             `reviewer_policy` back to their defaults. Returns 400 on out-of-range \
             or unknown-enum violations before any DB write.",
        )
        .response::<200, Json<AgentAdminSettings>>()
        .response_with::<400, (), _>(|res| {
            res.description(
                "Validation failed (out-of-range token cap / max-steps / fan-out, \
                 unknown sandbox/approval mode, or oversized reviewer_policy).",
            )
        })
}
