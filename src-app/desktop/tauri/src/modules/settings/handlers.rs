//! Settings Handlers
//!
//! HTTP route handlers for desktop settings management

use crate::core::DesktopRepos;
use axum::extract::Path;
use serde::{Deserialize, Serialize};
use ziee_chat::{Json, StatusCode};

#[derive(Serialize)]
pub struct SettingResponse {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Serialize)]
pub struct AllSettingsResponse {
    pub settings: Vec<SettingItem>,
}

#[derive(Serialize)]
pub struct SettingItem {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct SetSettingRequest {
    pub value: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
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
