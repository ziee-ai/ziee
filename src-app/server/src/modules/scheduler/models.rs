//! Data types for the scheduler module: the `scheduled_tasks` row + the
//! create/update request shapes. Enum-like columns are stored as `TEXT`
//! (`sqlx::FromRow` reads them as `String`, mirroring `workflow::WorkflowRun`);
//! `schedule.rs::ScheduleKind` is the typed view used by the engine.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::schedule::ScheduleKind;

/// Max task name length (validated in the handler; DB column is VARCHAR(255)).
pub const MAX_NAME_LEN: usize = 255;
/// Max prompt length for a `prompt`-kind task.
pub const MAX_PROMPT_LEN: usize = 32_768;
/// Cap on the per-task unattended allow-list (avoids an unbounded blob).
pub const MAX_ALLOWED_TOOLS: usize = 100;

/// One entry in a task's `allowed_unattended_tools` allow-list (DEC-17.4): an MCP
/// server the creator pre-authorizes to run unattended, optionally narrowed to a
/// single tool. `tool_name = None` allow-lists the whole server.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AllowedTool {
    pub server_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// One entry in a run's `skipped_tools` report (DEC-17.5): a tool that was NOT run
/// during an unattended firing, with the reason.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SkippedTool {
    pub tool_name: String,
    pub reason: String,
}

/// Parse a JSONB `allowed_unattended_tools` value into typed entries (tolerant:
/// an unexpected shape yields an empty list rather than erroring a read path).
pub fn parse_allowed_tools(v: &serde_json::Value) -> Vec<AllowedTool> {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// A row of `scheduled_tasks`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub enabled: bool,
    /// Set when AUTO-paused (`max_failures` / `conversation_deleted` /
    /// `target_missing`); NULL for a user enable/disable.
    pub paused_reason: Option<String>,

    // Target.
    pub target_kind: String, // 'workflow' | 'prompt'
    pub workflow_id: Option<Uuid>,
    pub inputs_json: serde_json::Value,
    pub assistant_id: Option<Uuid>,
    pub prompt: Option<String>,
    pub model_id: Option<Uuid>,

    // Schedule.
    pub schedule_kind: String, // 'once' | 'recurring'
    pub run_at: Option<DateTime<Utc>>,
    pub cron_expr: Option<String>,
    pub timezone: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,

    // Failure handling.
    pub consecutive_failures: i32,

    // Delivery + change-detection.
    pub notify_mode: String, // 'always' | 'silent'
    pub notify_on: String,   // 'always' | 'on_change'
    pub last_result_fingerprint: Option<String>,
    pub last_result_signature_json: Option<serde_json::Value>,

    // prompt-kind bound conversation.
    pub bound_conversation_id: Option<Uuid>,

    /// Per-task unattended tool allow-list (DEC-17). JSONB array of `AllowedTool`;
    /// read via `parse_allowed_tools`. Empty ⇒ built-in read-only tools only.
    pub allowed_unattended_tools: serde_json::Value,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ScheduledTask {
    /// The typed schedule kind (defaults to `Once` on an unexpected string,
    /// which the DB CHECK makes unreachable).
    pub fn schedule_kind(&self) -> ScheduleKind {
        match self.schedule_kind.as_str() {
            "recurring" => ScheduleKind::Recurring,
            _ => ScheduleKind::Once,
        }
    }

    /// True while the task is a live schedule candidate.
    pub fn is_active(&self) -> bool {
        self.enabled && self.paused_reason.is_none()
    }
}

/// Create-task request body.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateScheduledTask {
    pub name: String,
    pub target_kind: String,
    pub workflow_id: Option<Uuid>,
    #[serde(default = "empty_object")]
    pub inputs_json: serde_json::Value,
    pub assistant_id: Option<Uuid>,
    pub prompt: Option<String>,
    pub model_id: Uuid,

    pub schedule_kind: String,
    pub run_at: Option<DateTime<Utc>>,
    pub cron_expr: Option<String>,
    #[serde(default = "default_timezone")]
    pub timezone: String,

    #[serde(default = "default_notify_mode")]
    pub notify_mode: String,
    #[serde(default = "default_notify_mode")]
    pub notify_on: String,

    /// Unattended tool allow-list (DEC-17). Defaults to empty (safe floor:
    /// built-in read-only tools only).
    #[serde(default)]
    pub allowed_unattended_tools: Vec<AllowedTool>,
}

/// Update-task request body (all fields optional; only present ones change).
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateScheduledTask {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub inputs_json: Option<serde_json::Value>,
    pub assistant_id: Option<Uuid>,
    pub prompt: Option<String>,
    pub model_id: Option<Uuid>,
    pub schedule_kind: Option<String>,
    pub run_at: Option<DateTime<Utc>>,
    pub cron_expr: Option<String>,
    pub timezone: Option<String>,
    pub notify_mode: Option<String>,
    pub notify_on: Option<String>,
    /// When present, replaces the task's unattended allow-list (DEC-17).
    pub allowed_unattended_tools: Option<Vec<AllowedTool>>,
}

/// A row of `scheduled_task_runs` — one per firing (the "Runs" history).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct ScheduledTaskRun {
    pub id: Uuid,
    pub scheduled_task_id: Uuid,
    pub user_id: Uuid,
    pub trigger: String, // 'schedule' | 'run_now' | 'catchup'
    pub status: String,  // 'completed' | 'no_change' | 'failed'
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub notification_id: Option<Uuid>,
    pub workflow_run_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    /// Tools skipped this firing because they weren't permitted unattended
    /// (DEC-17.5). JSONB array of `SkippedTool`; `[]` when none.
    pub skipped_tools: serde_json::Value,
    pub fired_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// Insert shape for a completed firing's audit row.
#[derive(Debug, Clone)]
pub struct NewTaskRun {
    pub scheduled_task_id: Uuid,
    pub user_id: Uuid,
    pub trigger: String,
    pub status: String,
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub notification_id: Option<Uuid>,
    pub workflow_run_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    /// Tools skipped this firing (DEC-17.5); empty when none.
    pub skipped_tools: Vec<SkippedTool>,
    pub fired_at: DateTime<Utc>,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_notify_mode() -> String {
    "always".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-27 (ITEM-14, partial): `parse_allowed_tools` — the parse that feeds the
    // unattended `mcp_config` server-constrain set built in `dispatch::dispatch_prompt`
    // — distinguishes a whole-server grant (`tool_name` absent) from a per-tool
    // grant, and is TOLERANT of a malformed JSONB blob (returns empty, never
    // errors a read path). NOTE: the full constrain set (built-in-read-only ∪
    // allow-listed) is assembled INLINE in dispatch_prompt and is not extracted as
    // a discrete helper, so only the allow-listed-parse half is unit-covered here.
    #[test]
    fn parse_allowed_tools_distinguishes_whole_server_and_per_tool() {
        let srv1 = Uuid::new_v4();
        let srv2 = Uuid::new_v4();

        // Whole-server grant (no tool_name) + a per-tool grant.
        let v = serde_json::json!([
            { "server_id": srv1 },
            { "server_id": srv2, "tool_name": "search" },
        ]);
        let parsed = parse_allowed_tools(&v);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0],
            AllowedTool { server_id: srv1, tool_name: None },
            "a missing tool_name = whole-server grant"
        );
        assert_eq!(
            parsed[1],
            AllowedTool {
                server_id: srv2,
                tool_name: Some("search".to_string())
            },
            "an explicit tool_name = per-tool grant"
        );
    }

    #[test]
    fn parse_allowed_tools_is_tolerant_of_malformed_input() {
        // A non-array (or otherwise unexpected) shape yields an empty list — a
        // read path must never error on a garbage JSONB blob.
        assert!(parse_allowed_tools(&serde_json::json!({})).is_empty());
        assert!(parse_allowed_tools(&serde_json::json!("nope")).is_empty());
        assert!(parse_allowed_tools(&serde_json::json!(null)).is_empty());
        // The safe floor: an empty array = read-only built-ins only.
        assert!(parse_allowed_tools(&serde_json::json!([])).is_empty());
    }
}
