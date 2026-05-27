//! Settings Handlers
//!
//! HTTP route handlers for desktop settings management

use crate::core::DesktopRepos;
use axum::extract::Path;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ziee::{Json, StatusCode, TransformOperation};

#[derive(Serialize, JsonSchema)]
pub struct SettingResponse {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct AllSettingsResponse {
    pub settings: Vec<SettingItem>,
}

#[derive(Serialize, JsonSchema)]
pub struct SettingItem {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetSettingRequest {
    pub value: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

/// OpenAPI documentation for get_all_settings endpoint
pub fn get_all_settings_docs(op: TransformOperation) -> TransformOperation {
    op.description("Get all desktop settings")
        .id("DesktopSettings.getAll")
        .tag("desktop-settings")
        .response::<200, Json<AllSettingsResponse>>()
}

/// Get all settings
pub async fn get_all_settings() -> Result<Json<AllSettingsResponse>, (StatusCode, String)> {
    let settings = DesktopRepos
        .settings
        .get_all()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items = settings
        .into_iter()
        .map(|(key, value)| SettingItem { key, value })
        .collect();

    Ok(Json(AllSettingsResponse { settings: items }))
}

/// OpenAPI documentation for get_setting endpoint
pub fn get_setting_docs(op: TransformOperation) -> TransformOperation {
    op.description("Get a specific desktop setting by key")
        .id("DesktopSettings.get")
        .tag("desktop-settings")
        .response::<200, Json<SettingResponse>>()
}

/// Get a setting by key
pub async fn get_setting(
    Path(key): Path<String>,
) -> Result<Json<SettingResponse>, (StatusCode, String)> {
    let value = DesktopRepos
        .settings
        .get(&key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(SettingResponse { key, value }))
}

/// OpenAPI documentation for set_setting endpoint
pub fn set_setting_docs(op: TransformOperation) -> TransformOperation {
    op.description("Set a desktop setting value")
        .id("DesktopSettings.set")
        .tag("desktop-settings")
        .response::<200, Json<SuccessResponse>>()
}

/// Set a setting value
pub async fn set_setting(
    Path(key): Path<String>,
    Json(request): Json<SetSettingRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    DesktopRepos
        .settings
        .set(&key, &request.value)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Setting '{}' updated", key),
    }))
}

/// OpenAPI documentation for delete_setting endpoint
pub fn delete_setting_docs(op: TransformOperation) -> TransformOperation {
    op.description("Delete a desktop setting")
        .id("DesktopSettings.delete")
        .tag("desktop-settings")
        .response::<200, Json<SuccessResponse>>()
}

/// Delete a setting
pub async fn delete_setting(
    Path(key): Path<String>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    let deleted = DesktopRepos
        .settings
        .delete(&key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(SuccessResponse {
            success: true,
            message: format!("Setting '{}' deleted", key),
        }))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("Setting '{}' not found", key),
        ))
    }
}
