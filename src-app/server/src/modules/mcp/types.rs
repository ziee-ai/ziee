// MCP types
#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::models::{McpServer, TransportType};

// =====================================================
// Request Types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub transport_type: TransportType,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,

    // http/sse transport
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,

    // http/sse transport
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,
}

// =====================================================
// Response Types
// =====================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct McpServerListResponse {
    pub servers: Vec<McpServer>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GroupMcpServersRequest {
    pub server_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerGroupsRequest {
    pub group_ids: Vec<Uuid>,
}

/// Response for getting system MCP servers assigned to a group
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GroupSystemServersResponse {
    pub servers: Vec<McpServer>,
}

/// Request to update system MCP servers for a group
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateGroupSystemServersRequest {
    pub server_ids: Vec<Uuid>,
}
