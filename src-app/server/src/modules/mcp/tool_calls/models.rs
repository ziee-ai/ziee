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
    /// A tool call made by a workflow `tool` step (the workflow ToolDispatcher).
    Workflow,
    /// A tool call made by a `run_js` script's injected `ziee.tools.*` host
    /// function (the js_tool executor). Intermediate results stay in the script;
    /// this row is how the call surfaces in tool-call history.
    Script,
}

impl McpToolCallSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpToolCallSource::Chat => "chat",
            McpToolCallSource::Rest => "rest",
            McpToolCallSource::Always => "always",
            McpToolCallSource::Approval => "approval",
            McpToolCallSource::Sampling => "sampling",
            McpToolCallSource::Workflow => "workflow",
            McpToolCallSource::Script => "script",
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
    /// E4: set when a workflow `tool` step made this call (else `None`).
    pub workflow_run_id: Option<Uuid>,
    /// ITEM-12 (DEC-12): the agent reviewer's risk classification
    /// (`low`/`high`/`critical`) for an approval-needing call; `None` otherwise.
    pub review_classification: Option<String>,
}

/// ITEM-17 / DEC-1: the admin-gated raw reveal of a recorded call's arguments.
///
/// `arguments_json` here is the RAW `tool_use.input` the model emitted, read from
/// the paired `message_contents` block — NOT the `mcp_tool_calls.arguments_json`
/// column, which was redacted before insert and therefore never held the raw
/// value. When the transcript block no longer exists the recorded (redacted)
/// arguments are returned instead, with `raw = false`.
#[derive(Clone, Serialize, JsonSchema)]
pub struct McpToolCallReveal {
    /// The tool-call row this reveal is for.
    pub id: Uuid,
    /// The unredacted arguments, when the source block is still present.
    pub arguments_json: serde_json::Value,
    /// `true` when `arguments_json` is the raw transcript value; `false` when the
    /// source block is gone and the recorded (already redacted) arguments were
    /// returned as a fallback.
    pub raw: bool,
}
/// Hand-written so the raw arguments can never reach a log.
///
/// `arguments_json` is BY CONSTRUCTION the unredacted `tool_use.input` — the
/// exact value the rest of this feature exists to keep off surfaces. A derived
/// `Debug` would reprint it into the long-retention log stream from any future
/// `tracing::debug!(?reveal)`, `dbg!`, or panic message, defeating both the
/// recorder's denylist and the deliberately value-free audit line in
/// `handlers.rs`. Coding guidelines §3.
impl std::fmt::Debug for McpToolCallReveal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolCallReveal")
            .field("id", &self.id)
            .field("raw", &self.raw)
            .field("arguments_json", &"[redacted]")
            .finish()
    }
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

/// ITEM-14: the timing of ONE `McpSession::call_tool`.
///
/// Captured at the SINGLE place a tool call is actually clocked
/// (`client/session.rs::call_tool`) and handed to BOTH consumers: the persisted
/// `mcp_tool_calls` row (via `record::build_record`) and the `mcpToolComplete`
/// SSE frame (via `helpers::send_tool_complete_event`). There is exactly one
/// clock; this type is how its reading travels outward, so a live rail step and
/// the stored history can never disagree about when a tool ran or for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolCallTiming {
    /// Wall-clock instant the underlying `tools/call` was dispatched.
    pub started_at: time::OffsetDateTime,
    /// Monotonic elapsed time of that dispatch, in milliseconds.
    pub elapsed_ms: i64,
}

impl ToolCallTiming {
    /// `started_at` as RFC 3339, the wire format the SSE frames use.
    /// `None` if formatting somehow fails (never in practice) so a timing bug can
    /// never fail a tool call.
    pub fn started_at_rfc3339(&self) -> Option<String> {
        self.started_at
            .format(&time::format_description::well_known::Rfc3339)
            .ok()
    }
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
    /// E4: the workflow run this call belongs to, when made by a workflow
    /// `tool` step. Stamped post-creation via `McpSession::set_workflow_run`
    /// (so the ~5 other `get_or_create_with_context` call sites are untouched).
    pub workflow_run_id: Option<Uuid>,
    /// ITEM-12 (DEC-12): the agent reviewer's risk classification for this call,
    /// stamped post-creation via `McpSession::set_review_classification`.
    pub review_classification: Option<String>,
}

#[cfg(test)]
mod js_tool_source_tests {
    use super::McpToolCallSource;

    // TEST-23: the Script source stringifies + serde-roundtrips as "script".
    #[test]
    fn script_source_as_str_and_serde() {
        assert_eq!(McpToolCallSource::Script.as_str(), "script");
        assert_eq!(
            serde_json::to_value(McpToolCallSource::Script).unwrap(),
            serde_json::json!("script")
        );
        let back: McpToolCallSource = serde_json::from_value(serde_json::json!("script")).unwrap();
        assert_eq!(back, McpToolCallSource::Script);
    }
}
