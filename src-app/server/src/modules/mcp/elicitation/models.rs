// Elicitation protocol types

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

/// Hard cap on the serialized size of an MCP server's `requestedSchema`.
/// The schema is untrusted (comes from the remote MCP server) and is rendered
/// into a form by the client, so an unbounded schema is a memory/DoS vector.
pub const MAX_REQUESTED_SCHEMA_BYTES: usize = 1024 * 1024; // 1 MiB

/// Bound an incoming elicitation `requestedSchema` to [`MAX_REQUESTED_SCHEMA_BYTES`].
/// Oversized schemas are dropped (replaced with a minimal error schema) rather
/// than forwarded to the client. Applied at every MCP ingress point that parses
/// `requestedSchema`.
pub fn cap_requested_schema(schema: serde_json::Value) -> serde_json::Value {
    let len = serde_json::to_string(&schema).map(|s| s.len()).unwrap_or(0);
    if len > MAX_REQUESTED_SCHEMA_BYTES {
        tracing::warn!(
            "elicitation requestedSchema is {len} bytes (> {MAX_REQUESTED_SCHEMA_BYTES} cap); dropping it"
        );
        serde_json::json!({
            "type": "object",
            "properties": {},
            "x-ziee-error": "requested schema exceeded the 1 MiB limit and was dropped"
        })
    } else {
        schema
    }
}

/// The user's answer to an elicitation request.
/// Stored in the in-memory registry and sent back to the MCP server as a JSON-RPC result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ElicitationResponse {
    /// "accept" | "decline" | "cancel"
    pub action: String,
    /// Field values — only present when action == "accept"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
}

/// Request body for POST /api/mcp/elicitation/{id}/respond
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RespondToElicitationRequest {
    /// "accept" | "decline" | "cancel"
    pub action: String,
    /// Field values — only present when action == "accept"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
}

/// Response body for POST /api/mcp/elicitation/{id}/respond
#[derive(Debug, Serialize, JsonSchema)]
pub struct RespondToElicitationResponse {
    pub success: bool,
}

/// Notification sent from http.rs → mcp.rs when an elicitation is created,
/// so the extension layer can persist the content block via Repos without
/// introducing DB access in the low-level MCP client.
#[derive(Debug)]
pub struct ElicitationStartedNotification {
    /// Per-elicitation UUID (matches the registry key and SSE event)
    pub elicitation_id: Uuid,
    /// Pre-generated UUID for the message_contents row
    pub content_id: Uuid,
    /// ID of the assistant message that owns this elicitation
    pub message_id: Option<Uuid>,
    /// Human-readable prompt from the MCP server
    pub message: String,
    /// JSON Schema (SEP-1330) describing the requested fields
    pub requested_schema: serde_json::Value,
    /// Display name of the MCP server
    pub server: String,
}
