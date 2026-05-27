//! Updater Handlers
//!
//! HTTP route handlers for application update management

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use tauri_plugin_updater::UpdaterExt;
use ziee::{Json, StatusCode};

use crate::core::get_app_handle;

/// Global update state for tracking download progress
pub static UPDATE_STATE: RwLock<UpdateState> = RwLock::new(UpdateState {
    checking: false,
    available: false,
    downloading: false,
    ready_to_install: false,
    version: None,
    notes: None,
    progress: None,
    error: None,
});

/// Global storage for downloaded update bytes
pub static UPDATE_BYTES: RwLock<Option<Vec<u8>>> = RwLock::new(None);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateState {
    pub checking: bool,
    pub available: bool,
    pub downloading: bool,
    pub ready_to_install: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub progress: Option<f32>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateCheckResponse {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateStatusResponse {
    pub status: UpdateState,
}

#[derive(Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    pub message: String,
}

/// Check for available updates
pub async fn check_for_updates() -> Result<Json<UpdateCheckResponse>, (StatusCode, String)> {
    // Reset state
    {
        let mut state = UPDATE_STATE.write().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;
        *state = UpdateState {
            checking: true,
            ..Default::default()
        };
    }

    // Clear any previously downloaded bytes
    {
        let mut bytes = UPDATE_BYTES.write().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;
        *bytes = None;
    }

    let handle = get_app_handle();

    let result = async {
        let updater = handle
            .updater()
            .map_err(|e| format!("Failed to get updater: {}", e))?;

        match updater.check().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                let notes = update.body.clone();

                // Update state
                {
                    let mut state = UPDATE_STATE
                        .write()
                        .map_err(|e| format!("Lock error: {}", e))?;
                    state.checking = false;
                    state.available = true;
                    state.version = Some(version.clone());
                    state.notes = notes.clone();
                }

                Ok(UpdateCheckResponse {
                    available: true,
                    version: Some(version),
                    notes,
                })
            }
            Ok(None) => {
                // No update available
                {
                    let mut state = UPDATE_STATE
                        .write()
                        .map_err(|e| format!("Lock error: {}", e))?;
                    state.checking = false;
                    state.available = false;
                }

                Ok(UpdateCheckResponse {
                    available: false,
                    version: None,
                    notes: None,
                })
            }
            Err(e) => {
                let error_msg = format!("Update check failed: {}", e);
                {
                    let mut state = UPDATE_STATE
                        .write()
                        .map_err(|e| format!("Lock error: {}", e))?;
                    state.checking = false;
                    state.error = Some(error_msg.clone());
                }
                Err(error_msg)
            }
        }
    }
    .await;

    result
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

/// Download the available update
pub async fn download_update() -> Result<Json<SimpleResponse>, (StatusCode, String)> {
    // Check if update is available
    {
        let state = UPDATE_STATE.read().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;

        if !state.available {
            return Err((
                StatusCode::BAD_REQUEST,
                "No update available. Check for updates first.".to_string(),
            ));
        }

        if state.downloading {
            return Err((
                StatusCode::BAD_REQUEST,
                "Download already in progress.".to_string(),
            ));
        }
    }

    // Mark as downloading
    {
        let mut state = UPDATE_STATE.write().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;
        state.downloading = true;
        state.progress = Some(0.0);
    }

    let handle = get_app_handle();

    // Start download in background
    let handle_clone = handle.clone();
    tauri::async_runtime::spawn(async move {
        let updater = match handle_clone.updater() {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("Failed to get updater: {}", e);
                if let Ok(mut state) = UPDATE_STATE.write() {
                    state.downloading = false;
                    state.error = Some(format!("Failed to get updater: {}", e));
                }
                return;
            }
        };

        match updater.check().await {
            Ok(Some(update)) => {
                // Download with progress tracking
                let downloaded = update
                    .download(
                        |chunk_length, content_length| {
                            if let Some(total) = content_length {
                                let progress = (chunk_length as f32 / total as f32) * 100.0;
                                if let Ok(mut state) = UPDATE_STATE.write() {
                                    state.progress = Some(progress);
                                }
                            }
                        },
                        || {
                            tracing::info!("Update download complete");
                        },
                    )
                    .await;

                match downloaded {
                    Ok(bytes) => {
                        // Store the downloaded bytes for installation
                        if let Ok(mut stored_bytes) = UPDATE_BYTES.write() {
                            *stored_bytes = Some(bytes);
                        }

                        if let Ok(mut state) = UPDATE_STATE.write() {
                            state.downloading = false;
                            state.ready_to_install = true;
                            state.progress = Some(100.0);
                        }
                        tracing::info!("Update downloaded and ready to install");
                    }
                    Err(e) => {
                        tracing::error!("Download failed: {}", e);
                        if let Ok(mut state) = UPDATE_STATE.write() {
                            state.downloading = false;
                            state.error = Some(format!("Download failed: {}", e));
                        }
                    }
                }
            }
            Ok(None) => {
                if let Ok(mut state) = UPDATE_STATE.write() {
                    state.downloading = false;
                    state.error = Some("No update available".to_string());
                }
            }
            Err(e) => {
                tracing::error!("Update check failed during download: {}", e);
                if let Ok(mut state) = UPDATE_STATE.write() {
                    state.downloading = false;
                    state.error = Some(format!("Update check failed: {}", e));
                }
            }
        }
    });

    Ok(Json(SimpleResponse {
        success: true,
        message: "Download started. Check status endpoint for progress.".to_string(),
    }))
}

/// Install the downloaded update and restart
pub async fn install_update() -> Result<Json<SimpleResponse>, (StatusCode, String)> {
    // Check if ready to install
    {
        let state = UPDATE_STATE.read().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;

        if !state.ready_to_install {
            return Err((
                StatusCode::BAD_REQUEST,
                "No update ready to install. Download first.".to_string(),
            ));
        }
    }

    // Get the downloaded bytes
    let bytes = {
        let stored_bytes = UPDATE_BYTES.read().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Lock error: {}", e),
            )
        })?;

        stored_bytes.clone().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Downloaded bytes not found".to_string(),
            )
        })?
    };

    let handle = get_app_handle();

    let updater = handle.updater().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get updater: {}", e),
        )
    })?;

    match updater.check().await {
        Ok(Some(update)) => {
            // Install will quit the app and restart
            tracing::info!("Installing update and restarting...");

            if let Err(e) = update.install(&bytes) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Install failed: {}", e),
                ));
            }

            // This won't be reached as install() restarts the app
            Ok(Json(SimpleResponse {
                success: true,
                message: "Installing update...".to_string(),
            }))
        }
        Ok(None) => Err((StatusCode::BAD_REQUEST, "No update available".to_string())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Update check failed: {}", e),
        )),
    }
}

/// Get current update status
pub async fn get_update_status() -> Result<Json<UpdateStatusResponse>, (StatusCode, String)> {
    let state = UPDATE_STATE.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Lock error: {}", e),
        )
    })?;

    Ok(Json(UpdateStatusResponse {
        status: state.clone(),
    }))
}
