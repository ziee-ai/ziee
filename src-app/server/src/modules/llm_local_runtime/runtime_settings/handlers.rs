//! REST handlers for runtime settings.
//!
//! `GET /api/local-runtime/settings` — `RuntimeSettingsRead`
//! `PUT /api/local-runtime/settings` — `RuntimeSettingsManage`

use aide::transform::TransformOperation;
use axum::{Json, http::StatusCode};

use super::models::{RuntimeSettings, UpdateRuntimeSettingsRequest};
use crate::common::ApiResult;
use crate::core::repository::Repos;
use crate::modules::llm_local_runtime::permissions::{
    RuntimeSettingsManage, RuntimeSettingsRead,
};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

/// GET /api/local-runtime/settings
pub async fn get_runtime_settings(
    _auth: RequirePermissions<(RuntimeSettingsRead,)>,
) -> ApiResult<Json<RuntimeSettings>> {
    let row = Repos.local_runtime.get_runtime_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_runtime_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RuntimeSettingsRead,)>(op)
        .id("LocalRuntime.getRuntimeSettings")
        .tag("LLM Local Runtime")
        .summary("Read runtime singleton settings (idle/auto-start/drain/allow_unsigned).")
        .response::<200, Json<RuntimeSettings>>()
}

/// PUT /api/local-runtime/settings
pub async fn update_runtime_settings(
    _auth: RequirePermissions<(RuntimeSettingsManage,)>,
    origin: SyncOrigin,
    Json(req): Json<UpdateRuntimeSettingsRequest>,
) -> ApiResult<Json<RuntimeSettings>> {
    let row = Repos.local_runtime.update_runtime_settings(req).await?;
    // Singleton settings (event id is nil); notify admin devices.
    sync_publish(
        SyncEntity::RuntimeSettings,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<RuntimeSettingsRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_runtime_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(RuntimeSettingsManage,)>(op)
        .id("LocalRuntime.updateRuntimeSettings")
        .tag("LLM Local Runtime")
        .summary("Update runtime singleton settings (PATCH-style: COALESCE on each field).")
        .response::<200, Json<RuntimeSettings>>()
        .response_with::<400, (), _>(|r| r.description("Out-of-range value"))
}
