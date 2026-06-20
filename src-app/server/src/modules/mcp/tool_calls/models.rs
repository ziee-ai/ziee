//! Types for the MCP tool-call history (`mcp_tool_calls`).
//!
//! Mirrors the `workflow_runs` shape: a row struct (chrono timestamps, for the
//! API) + an insert payload (`time::OffsetDateTime` timestamps, for binding).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Terminal status of a recorded tool call. Calls are synchronous, so there is
/// no pending/running state — the row is written once, on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    Completed,
    Failed,
    Timeout,
    /// Reserved: explicit mid-call cancellation isn't surfaced today (the rare
    /// outer-timeout cancel in `execute_tool` drops the record entirely rather
    /// than recording `cancelled`). Kept as a valid terminal state for the
    /// schema CHECK + forward compatibility.
    Cancelled,
}

impl McpToolCallStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpToolCallStatus::Completed => "completed",
            McpToolCallStatus::Failed => "failed",
            McpToolCallStatus::Timeout => "timeout",
            McpToolCallStatus::Cancelled => "cancelled",
        }
    }
}

/// How a tool call was triggered. Drives the source badge + filters in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallSource {
    /// LLM-driven tool call in the chat after_llm_call loop (the common case).
    #[default]
    Chat,
    /// Manual REST call via `POST /api/mcp/servers/{id}/tools/{name}/call`.
    Rest,
    /// A `usage_mode = always` tool pre-run in before_llm_call.
    Always,
    /// A previously-approved tool executed on the approval-resume path.
    Approval,
    /// A tool call made by a sampling-capable session.
    Sampling,
}

impl McpToolCallSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpToolCallSource::Chat => "chat",
            McpToolCallSource::Rest => "rest",
            McpToolCallSource::Always => "always",
            McpToolCallSource::Approval => "approval",
            McpToolCallSource::Sampling => "sampling",
        }
    }
}

/// One recorded MCP tool-call invocation (a `mcp_tool_calls` row).
///
/// `source`/`status` are stored (and surfaced) as their snake_case strings.
/// Field ORDER must match the SELECT/RETURNING column list in `repository.rs`
/// (`query_as!` maps by name, but keeping them aligned aids review).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct McpToolCall {
    pub id: Uuid,
    pub server_id: Option<Uuid>,
    pub server_name: String,
    pub is_built_in: bool,
    pub user_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub tool_use_id: Option<String>,
    pub tool_name: String,
    pub arguments_json: serde_json::Value,
    pub source: String,
    pub status: String,
    pub is_error: bool,
    pub result_json: Option<serde_json::Value>,
    pub content_kinds: Vec<String>,
    pub result_bytes: i64,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insert payload for a recorded tool call. Timestamps are
/// `time::OffsetDateTime` so they bind directly to TIMESTAMPTZ (the codebase's
/// proven bind type; the row reads them back as chrono via `as "col: _"`).
#[derive(Debug, Clone)]
pub struct CreateMcpToolCall {
    pub server_id: Option<Uuid>,
    pub server_name: String,
    pub is_built_in: bool,
    pub user_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub tool_use_id: Option<String>,
    pub tool_name: String,
    pub arguments_json: serde_json::Value,
    pub source: McpToolCallSource,
    pub status: McpToolCallStatus,
    pub is_error: bool,
    pub result_json: Option<serde_json::Value>,
    pub content_kinds: Vec<String>,
    pub result_bytes: i64,
    pub error_message: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub duration_ms: Option<i64>,
}

/// Paginated list response (mirrors `McpServerListResponse`).
#[derive(Debug, Serialize, JsonSchema)]
pub struct McpToolCallListResponse {
    pub calls: Vec<McpToolCall>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

/// Context stamped onto an `McpSession` at creation so `call_tool` can record
/// who/where/how on every path. `user_id` is `None` only for an unstamped
/// (pooled, non-tool-call) session — recording is skipped in that case so we
/// never insert a row without an owner.
#[derive(Debug, Clone, Default)]
pub struct McpCallContext {
    pub user_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub tool_use_id: Option<String>,
    pub source: McpToolCallSource,
    pub server_name: String,
    pub is_built_in: bool,
}
