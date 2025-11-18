use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

use super::client::{Tool, Resource, ToolContent};

// Tool types
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListToolsResponse {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CallToolRequest {
    pub arguments: serde_json::Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CallToolResponse {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

// Resource types
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListResourcesResponse {
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadResourceRequest {
    pub uri: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadResourceResponse {
    pub content: serde_json::Value,
}

// Session types
#[derive(Debug, Serialize, JsonSchema)]
#[allow(dead_code)] // Future feature: server status endpoint for monitoring
pub struct ServerStatusResponse {
    pub server_id: Uuid,
    pub connected: bool,
    pub age_seconds: Option<u64>,
    pub idle_seconds: Option<u64>,
}
