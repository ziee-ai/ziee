// LLM Model API types - request/response types for API communication
// Separated from models.rs to distinguish API types from database entities

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{
    DownloadProgressData, DownloadRequestData, DownloadStatus, EngineType, FileFormat,
    ModelCapabilities, ModelEngineSettings, ModelParameters, SourceInfo,
};

// =====================================================
// REQUEST TYPES
// =====================================================

/// Request to create a new LLM model
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLlmModelRequest {
    pub provider_id: Uuid,
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ModelParameters>,
    pub engine_type: EngineType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_settings: Option<ModelEngineSettings>,
    pub file_format: FileFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,
}

/// Request to update an existing LLM model
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateLlmModelRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ModelParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_type: Option<EngineType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_settings: Option<ModelEngineSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_format: Option<FileFormat>,
}

// Default implementation for UpdateLlmModelRequest
impl Default for UpdateLlmModelRequest {
    fn default() -> Self {
        Self {
            name: None,
            display_name: None,
            description: None,
            enabled: None,
            is_active: None,
            capabilities: None,
            parameters: None,
            engine_type: None,
            engine_settings: None,
            file_format: None,
        }
    }
}

// =====================================================
// RESPONSE TYPES
// =====================================================

/// Response for listing LLM models with pagination
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmModelListResponse {
    pub models: Vec<super::models::LlmModel>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

/// Response for download instance list
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DownloadInstanceListResponse {
    pub downloads: Vec<super::models::DownloadInstance>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

// =====================================================
// QUERY TYPES
// =====================================================

/// Query parameters for listing models with optional provider filtering
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsQuery {
    /// Optional provider ID to filter models by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    10
}

// =====================================================
// DOWNLOAD REQUEST/RESPONSE TYPES
// =====================================================

// Note: DownloadRequestData and DownloadProgressData are defined in models.rs
// because they are stored as JSON in the database and are part of the database schema

/// Request to create a new download instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadInstanceRequest {
    pub provider_id: Uuid,
    pub repository_id: Uuid,
    pub request_data: DownloadRequestData,
}

/// Request to update download progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDownloadProgressRequest {
    pub progress_data: DownloadProgressData,
    pub status: Option<DownloadStatus>,
}

/// Request to update download status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDownloadStatusRequest {
    pub status: DownloadStatus,
    pub error_message: Option<String>,
    pub model_id: Option<Uuid>,
}
