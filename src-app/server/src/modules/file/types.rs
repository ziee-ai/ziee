// File API types

use crate::modules::file::models::File;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// File list response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FileListResponse {
    pub files: Vec<File>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

/// Download token response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadTokenResponse {
    pub token: String,
    pub expires_in: i64, // Seconds until expiration
}

/// Pagination query params
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_per_page")]
    pub per_page: i32,
}

fn default_page() -> i32 {
    1
}

fn default_per_page() -> i32 {
    20
}

/// Preview query params
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewQuery {
    #[serde(default = "default_preview_page")]
    pub page: u32,
}

fn default_preview_page() -> u32 {
    1
}

/// Download token query params
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadTokenQuery {
    pub token: String,
}

/// JWT claims for download tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadTokenClaims {
    pub file_id: String,
    pub user_id: String,
    pub exp: usize,
    pub iat: usize,
}
