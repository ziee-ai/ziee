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

/// Text page query params
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TextPageQuery {
    pub page: Option<u32>, // If None, return all pages concatenated
}

/// Download token query params
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadTokenQuery {
    pub token: String,
}

/// JWT claims for download tokens.
///
/// Carries `iss` / `aud` so a download token can NOT be cross-used as
/// an access token even though both are signed with the same secret —
/// the access-token validator requires `aud="ziee-chat-api"`, ours
/// requires `aud="ziee-chat-download"`. Closes 02-permissions F-03.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTokenClaims {
    pub file_id: String,
    pub user_id: String,
    pub exp: usize,
    pub iat: usize,
    /// Issuer — same value as the access-token issuer for now; can
    /// diverge if/when we split signers.
    pub iss: String,
    /// Audience — MUST be `"ziee-chat-download"` for download tokens.
    /// The validator rejects any other value; cross-audience replay
    /// is impossible.
    pub aud: String,
}

/// Audience claim value for download-only tokens. Distinct from the
/// access-token audience (`ziee-chat-api`) so a leaked download token
/// can't be replayed against authenticated endpoints.
pub const DOWNLOAD_TOKEN_AUDIENCE: &str = "ziee-chat-download";

/// Helper type for documenting binary responses in OpenAPI
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BlobType {}
