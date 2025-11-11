use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =====================================================
// Request/Response Types
// =====================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
    pub app_name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetupAdminRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}
