//! Wire types for the MCP user-policy singleton.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The global MCP user-policy row (singleton; id always = 1 in the DB).
///
/// Drives what regular users can install:
///   - `allowed_transports == []` → no Add button, no Hub MCP tab for
///     non-admins.
///   - `'stdio' ∈ allowed_transports` → user-installed stdio MCP rows
///     get `run_in_sandbox = true` + `sandbox_flavor =
///     user_stdio_sandbox_flavor` force-applied at create/update time.
///
/// Admin-only edits via PUT /api/mcp/user-policy (perm
/// `McpUserPolicyEdit`); read is open to any authenticated user (the
/// UI needs it to gate the Add button + Hub tab visibility).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpUserPolicy {
    /// Subset of `{"http", "stdio"}`. `sse` is admin-only (not
    /// exposed in the user-mode drawer dropdown).
    pub allowed_transports: Vec<String>,
    /// Required iff `"stdio" ∈ allowed_transports`; must be in
    /// `code_sandbox::types::KNOWN_FLAVORS`.
    pub user_stdio_sandbox_flavor: Option<String>,
    /// Days to retain MCP tool-call history (`mcp_tool_calls`) before the
    /// background prune deletes it. `0` = keep forever (no prune).
    pub tool_call_retention_days: i32,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

/// PUT body. `updated_by` is taken from the auth context, not the
/// request.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateMcpUserPolicyRequest {
    pub allowed_transports: Vec<String>,
    pub user_stdio_sandbox_flavor: Option<String>,
    /// Days to retain MCP tool-call history; `0` = keep forever. Clamped to
    /// [0, 3650] on save. Omitted (`None`) keeps the current value, so
    /// existing PUT callers that don't set it are unaffected.
    #[serde(default)]
    pub tool_call_retention_days: Option<i32>,
}
