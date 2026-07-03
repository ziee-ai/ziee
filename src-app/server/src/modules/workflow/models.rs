//! Workflow + WorkflowRun database row + create/update payloads.
//!
//! Mirrors the `workflows` + `workflow_runs` tables (migration
//! `00000000000095_create_skills_and_workflows_tables.sql`). Bundle
//! content (workflow.yaml, prompts/, scripts/, references/) lives on
//! disk under `extracted_path`; the runner's per-run staging is under
//! `<workspace>/<conv>/workflow/<run>/`.


use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database row in `workflows`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub extracted_path: String,
    pub bundle_sha256: String,
    pub bundle_size_bytes: i64,
    pub file_count: i32,
    pub entry_point: String,
    pub tags: serde_json::Value,
    pub scope: String, // 'user' | 'system'
    pub owner_user_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub enabled: bool,
    pub is_dev: bool,
    /// An LLM-authored, conversation-scoped throwaway workflow materialized by
    /// the `run_from_workspace` verb. Excluded from every listing (never a
    /// `wf_<slug>` tool nor on the workflows page) — it only runs via the
    /// generic verb. CASCADE-cleaned with `conversation_id`.
    pub ephemeral: bool,
    /// The owning conversation for an `ephemeral` row (else NULL). Set so the
    /// row (and its runs) GC when the conversation is deleted.
    pub conversation_id: Option<Uuid>,
    /// Pre-resolved templates + typed step metadata. NULL until the
    /// validator's compile pass runs (B4). See plan §4.1 pattern (d).
    pub compiled_ir_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkflow {
    pub name: String,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub extracted_path: String,
    pub bundle_sha256: String,
    pub bundle_size_bytes: i64,
    pub file_count: i32,
    pub entry_point: String,
    pub tags: serde_json::Value,
    pub scope: String,
    pub owner_user_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub enabled: bool,
    pub is_dev: bool,
    pub ephemeral: bool,
    pub conversation_id: Option<Uuid>,
    pub compiled_ir_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateWorkflow {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub tags: Option<serde_json::Value>,
}

/// Per-conversation OPT-OUT row (mirrors `conversation_skill_overrides`).
/// Phase B6 may add this as its own table if workflows need
/// conversation-scoped hides; for now the type is reserved for parity
/// with skills.
// Reserved for parity with `conversation_skill_overrides`; no consumer until
// workflows need conversation-scoped hides (see doc above).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationWorkflowOverride {
    pub conversation_id: Uuid,
    pub workflow_id: Uuid,
    pub hidden: bool,
}

/// Terminal-or-in-flight state of one `workflow_runs` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Pending,
    Running,
    /// Non-terminal: parked on an `elicit` human gate with no resident runner.
    /// Spared by the boot sweep and resumed lazily when the human submits
    /// (durable resume).
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowRunStatus::Pending => "pending",
            WorkflowRunStatus::Running => "running",
            WorkflowRunStatus::Waiting => "waiting",
            WorkflowRunStatus::Completed => "completed",
            WorkflowRunStatus::Failed => "failed",
            WorkflowRunStatus::Cancelled => "cancelled",
        }
    }

    /// A run in a terminal state will never transition again.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkflowRunStatus::Completed
                | WorkflowRunStatus::Failed
                | WorkflowRunStatus::Cancelled
        )
    }
}

/// Database row in `workflow_runs`. Heavy fields (step outputs, logs,
/// artifacts, final output) live as JSONB metadata blobs — actual
/// content (multi-MiB step output, artifact bytes) is on disk under
/// the per-run workspace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub user_id: Uuid,
    pub model_id: Option<Uuid>,
    pub sandbox_flavor: Option<String>,
    pub run_kind: String, // 'normal' | 'test' | 'dry_run'
    pub inputs_json: serde_json::Value,
    pub step_outputs_json: serde_json::Value,
    pub step_item_progress_json: serde_json::Value,
    pub step_logs_json: serde_json::Value,
    pub step_artifacts_json: serde_json::Value,
    pub pending_elicitation_json: Option<serde_json::Value>,
    pub final_output_json: Option<serde_json::Value>,
    /// P2.6: the running sandbox step's live progress track map
    /// (`{ id -> ProgressTrack }`), or NULL when no step is streaming progress.
    /// Drives Snapshot rehydration of in-flight bars after a refresh.
    pub step_progress_json: Option<serde_json::Value>,
    pub status: String,
    pub current_step: Option<String>,
    pub error_message: Option<String>,
    pub total_tokens: i64, // M4: BIGINT column — a long run can exceed i32 range
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkflowRun {
    pub workflow_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub user_id: Uuid,
    pub model_id: Option<Uuid>,
    pub sandbox_flavor: Option<String>,
    pub run_kind: String,
    /// `"manual"` (REST /run, workflow page) or `"conversation"` (LLM tool
    /// call). Drives the run-history "trigger" badge.
    pub invocation_source: String,
    pub inputs_json: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_status_roundtrips_and_is_non_terminal() {
        // Change B: the durable-resume `waiting` status serializes as "waiting"
        // (matching the migration-110 CHECK + the SQL status filters) and is
        // classified non-terminal so the boot sweep / cancel paths treat it as
        // resumable, not done.
        assert_eq!(WorkflowRunStatus::Waiting.as_str(), "waiting");
        assert!(!WorkflowRunStatus::Waiting.is_terminal());
        // Sanity: the terminal trio is terminal; the in-flight trio is not.
        assert!(WorkflowRunStatus::Completed.is_terminal());
        assert!(WorkflowRunStatus::Failed.is_terminal());
        assert!(WorkflowRunStatus::Cancelled.is_terminal());
        assert!(!WorkflowRunStatus::Pending.is_terminal());
        assert!(!WorkflowRunStatus::Running.is_terminal());
    }
}
