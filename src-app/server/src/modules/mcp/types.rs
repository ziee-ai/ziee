// MCP types
#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::models::{McpServer, SetMcpServerOAuthConfigRequest, TransportType, UsageMode};

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

    // Sampling configuration
    pub supports_sampling: Option<bool>,
    pub usage_mode: Option<UsageMode>,
    pub max_concurrent_sessions: Option<i32>,
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

    // Sampling configuration
    pub supports_sampling: Option<bool>,
    pub usage_mode: Option<UsageMode>,
    pub max_concurrent_sessions: Option<i32>,
}

/// Request to test an MCP server connection without persisting anything.
///
/// Carries the same transport fields as a create/update request so the UI can
/// probe the *current form values* before saving. `oauth` is the credentials
/// typed into the form (new external server); since the client secret is
/// write-only in the edit / list flows, `id` lets the server fall back to the
/// stored OAuth config for that existing server.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestMcpConnectionRequest {
    pub transport_type: TransportType,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,

    // http transport
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,

    /// OAuth client_credentials typed into the form (new external HTTP server).
    pub oauth: Option<SetMcpServerOAuthConfigRequest>,

    /// Existing server id — used ONLY to recover the stored OAuth secret when
    /// `oauth` is absent (edit drawer / list card). Access-checked before use.
    pub id: Option<Uuid>,
}

// =====================================================
// Response Types
// =====================================================

/// Result of a connection test — `success` is the only authoritative field.
/// On failure `message` carries the underlying error (timeout / 401 / bad
/// command). On success `tool_count` is the number of tools the server
/// advertised (best-effort; `None` if the handshake succeeded but listing
/// tools failed).
#[derive(Debug, Serialize, JsonSchema)]
pub struct TestMcpConnectionResponse {
    pub success: bool,
    pub message: String,
    pub tool_count: Option<usize>,
}

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
