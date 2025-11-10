use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use super::models::{HubModel, HubAssistant, HubMCPServer};

/// Query parameters for hub endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HubQuery {
    /// Locale code (e.g., "en", "es", "fr")
    #[serde(default = "default_locale")]
    pub lang: String,
}

fn default_locale() -> String {
    "en".to_string()
}

/// Version response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubVersionResponse {
    pub version: String,
    pub last_updated: Option<String>,
}

/// Refresh response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HubRefreshResponse {
    pub updated: bool,
    pub version: String,
}

/// Response types (for OpenAPI)
pub type HubModelsResponse = Vec<HubModel>;
pub type HubAssistantsResponse = Vec<HubAssistant>;
pub type HubMCPServersResponse = Vec<HubMCPServer>;
