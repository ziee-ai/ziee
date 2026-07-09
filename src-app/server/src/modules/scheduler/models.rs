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
