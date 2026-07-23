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

/// Response of `POST /api/workflows/validate-def` — structured validation
/// findings (split by severity) plus a dry-run cost estimate for a posted
/// `WorkflowDef`. Returned with a 200 even when `errors` is non-empty.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidateDefResponse {
    pub errors: Vec<crate::modules::workflow::validate::ValidationError>,
    pub warnings: Vec<crate::modules::workflow::validate::ValidationError>,
    pub cost_estimate: crate::modules::workflow::cost::DryRunResult,
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
    /// Non-terminal (ITEM-17): a `kind: agent` run that crashed mid-loop while
    /// `resumable_agent = true`. The boot sweep marks it `resumable` (NOT
    /// `failed`) and re-drives it via `resume_run`, which replays the persisted
    /// `agent_transcript_json` so completed tool calls are not re-run.
    Resumable,
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
            WorkflowRunStatus::Resumable => "resumable",
            WorkflowRunStatus::Completed => "completed",
            WorkflowRunStatus::Failed => "failed",
            WorkflowRunStatus::Cancelled => "cancelled",
        }
    }

    /// Parse the DB `status` text back into the enum. Returns `None` for an
    /// unrecognized value (callers treat that as non-terminal / in-flight).
    pub fn from_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => WorkflowRunStatus::Pending,
            "running" => WorkflowRunStatus::Running,
            "waiting" => WorkflowRunStatus::Waiting,
            "resumable" => WorkflowRunStatus::Resumable,
            "completed" => WorkflowRunStatus::Completed,
            "failed" => WorkflowRunStatus::Failed,
            "cancelled" => WorkflowRunStatus::Cancelled,
            _ => return None,
        })
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

/// Orthogonal discriminator on a `workflow_runs` row (ITEM-14 / DEC-22/23,
/// LOCK-2 hybrid): WHICH background-run substrate produced the row. Distinct
/// from `run_kind` (normal/test/dry_run) and `invocation_source`
/// (manual/conversation/agent/...) — a background run can be any combination.
///
/// - `Workflow` — the classic YAML-DAG run: has a backing `workflows` row and an
///   on-disk `workflow.yaml`; `workflow_id` is non-NULL.
/// - `SandboxExec` — a fire-and-forget background command (no bundle).
/// - `SubAgent` — a detached agent-core turn (Option C); resumes via transcript
///   replay.
///
/// The generalized background kinds (`SandboxExec`, `SubAgent`) reuse the
/// runner's spawn / heartbeat / `RunHandle` / startup-sweep machinery with
/// `workflow_id = NULL` and no bundle. Each kind's sweep / flap / retention
/// policy lives in the decentralized [`super::job_kind`] registry, never a
/// central `match`.
// Seam (ITEM-17): the model-facing check_status/collect_result MCP trio + the
// sub-agent/sandbox background drivers (a later tranche) construct + parse
// `JobKind`; the backbone wires it into production today only as `job_kind` text
// + the sweep policy registry. Allowed until that tranche adds a live caller.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Workflow,
    SandboxExec,
    SubAgent,
}

// `from_db_str` / `is_background` are the parse seam the model-facing
// check_status/collect_result MCP trio uses in a later tranche; `as_str` is used
// by the create path. Allowed until that tranche lands a live caller.
#[allow(dead_code)]
impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::Workflow => "workflow",
            JobKind::SandboxExec => "sandbox_exec",
            JobKind::SubAgent => "subagent",
        }
    }

    /// Parse the DB `job_kind` text. Returns `None` for an unrecognized value
    /// (forward-compat: a newer server may persist a kind an older binary can't
    /// name; the caller treats an unknown kind conservatively rather than
    /// panicking — mirrors [`WorkflowRunStatus::from_db_str`]).
    pub fn from_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "workflow" => JobKind::Workflow,
            "sandbox_exec" => JobKind::SandboxExec,
            "subagent" => JobKind::SubAgent,
            _ => return None,
        })
    }

    /// A non-`workflow` background kind (no bundle, `workflow_id = NULL`).
    pub fn is_background(&self) -> bool {
        !matches!(self, JobKind::Workflow)
    }
}

/// Database row in `workflow_runs`. Heavy fields (step outputs, logs,
/// artifacts, final output) live as JSONB metadata blobs — actual
/// content (multi-MiB step output, artifact bytes) is on disk under
/// the per-run workspace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct WorkflowRun {
    pub id: Uuid,
    /// NULL for a generalized background run (`job_kind != 'workflow'`) — a
    /// sub-agent turn / sandbox exec has no backing `workflows` bundle
    /// (ITEM-14 / DEC-22). Always set for a classic `workflow`-kind run.
    pub workflow_id: Option<Uuid>,
    /// Orthogonal background-run discriminator (raw DB text; parse with
    /// [`JobKind::from_db_str`]). `'workflow'` for the classic YAML-DAG run.
    /// Kept as `String` (not the enum) so an unknown value from a newer server
    /// round-trips without a deserialization failure — same posture as `status`.
    pub job_kind: String,
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

/// Create payload for a generalized BACKGROUND run (ITEM-14 / ITEM-17): a
/// non-`workflow` `JobKind` row with `workflow_id = NULL`. The row starts
/// `pending`; [`super::runner::spawn_background_run`] registers a `RunHandle`,
/// `mark_running`s it, and drives it to terminal reusing the same guards as the
/// workflow runner. `run_kind` is fixed to `'normal'` (a background run is never
/// a test/dry-run of a workflow).
// Seam (ITEM-17): constructed by the background drivers / MCP `spawn_background`
// in a later tranche; the backbone ships the create path + tests.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CreateBackgroundRun {
    /// MUST be a background kind (`SandboxExec` / `SubAgent`); a `Workflow` kind
    /// goes through [`CreateWorkflowRun`] + `insert_run` (it has a bundle).
    pub job_kind: JobKind,
    pub conversation_id: Option<Uuid>,
    pub user_id: Uuid,
    pub model_id: Option<Uuid>,
    pub sandbox_flavor: Option<String>,
    /// One of the `workflow_runs_invocation_source_check` values
    /// (`conversation` / `agent` / ...). Names the trigger for the history view.
    pub invocation_source: String,
    pub inputs_json: serde_json::Value,
}

/// One durable STEERING NOTE queued against a running background run (ITEM-25 /
/// Group F). A user posts a note to a RUNNING background run (a detached
/// `JobKind::SubAgent` turn); the detached agent-core loop consumes pending notes
/// at its next iteration boundary and appends them to the transcript so the model
/// reads them on the next turn. `consumed_at` is NULL while pending, stamped when
/// the loop consumes it. Rows live in `background_run_notes`, FK'd to the run
/// (ON DELETE CASCADE) — deleting the run deletes its notes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct RunNote {
    pub id: Uuid,
    pub run_id: Uuid,
    /// The steering text the running agent should pick up next turn.
    pub note: String,
    pub created_at: DateTime<Utc>,
    /// NULL while pending; set to the consume time once the loop reads it.
    pub consumed_at: Option<DateTime<Utc>>,
}

/// Enqueue-a-steering-note request body
/// (`POST /api/background/runs/{run_id}/notes`). The note is capped + trimmed by
/// the handler before it reaches the durable queue.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateRunNote {
    /// The steering note the running agent should pick up on its next turn
    /// (non-empty; capped at 4000 characters by the handler).
    pub note: String,
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
        // ITEM-17: `resumable` roundtrips + is non-terminal (crash-resume state).
        assert_eq!(WorkflowRunStatus::Resumable.as_str(), "resumable");
        assert_eq!(
            WorkflowRunStatus::from_db_str("resumable"),
            Some(WorkflowRunStatus::Resumable)
        );
        assert!(!WorkflowRunStatus::Resumable.is_terminal());
        // Sanity: the terminal trio is terminal; the in-flight trio is not.
        assert!(WorkflowRunStatus::Completed.is_terminal());
        assert!(WorkflowRunStatus::Failed.is_terminal());
        assert!(WorkflowRunStatus::Cancelled.is_terminal());
        assert!(!WorkflowRunStatus::Pending.is_terminal());
        assert!(!WorkflowRunStatus::Running.is_terminal());
    }

    // TEST-48 (ITEM-14): `job_kind` parses / round-trips and is ORTHOGONAL to
    // `run_kind` + `invocation_source` — none of the three vocabularies overlap,
    // so a background run can freely combine them.
    #[test]
    fn job_kind_parses_round_trips_and_is_orthogonal() {
        // Round-trip every kind through as_str <-> from_db_str.
        for k in [JobKind::Workflow, JobKind::SandboxExec, JobKind::SubAgent] {
            assert_eq!(JobKind::from_db_str(k.as_str()), Some(k));
        }
        assert_eq!(JobKind::from_db_str("workflow"), Some(JobKind::Workflow));
        assert_eq!(
            JobKind::from_db_str("sandbox_exec"),
            Some(JobKind::SandboxExec)
        );
        assert_eq!(JobKind::from_db_str("subagent"), Some(JobKind::SubAgent));
        // Forward-compat: an unknown kind is `None`, never a panic (TEST-132 twin).
        assert_eq!(JobKind::from_db_str("future_kind"), None);
        assert_eq!(JobKind::from_db_str(""), None);

        // Background classification.
        assert!(!JobKind::Workflow.is_background());
        assert!(JobKind::SandboxExec.is_background());
        assert!(JobKind::SubAgent.is_background());

        // Orthogonality: the `job_kind` vocabulary is DISJOINT from the
        // `run_kind` and `invocation_source` vocabularies (migration CHECKs) —
        // proving `job_kind` is a genuinely new axis, not an overload of either.
        let run_kinds = ["normal", "test", "dry_run"];
        let invocation_sources = ["manual", "conversation", "agent", "mcp_tool", "scheduled"];
        for k in [JobKind::Workflow, JobKind::SandboxExec, JobKind::SubAgent] {
            let s = k.as_str();
            assert!(
                !run_kinds.contains(&s),
                "job_kind '{s}' must not collide with a run_kind value"
            );
            assert!(
                !invocation_sources.contains(&s),
                "job_kind '{s}' must not collide with an invocation_source value"
            );
        }
        // JSON (serde) form matches the DB text (snake_case rename) so the wire /
        // OpenAPI representation and the persisted value are the same token.
        assert_eq!(
            serde_json::to_value(JobKind::SandboxExec).unwrap(),
            serde_json::json!("sandbox_exec")
        );
        assert_eq!(
            serde_json::from_value::<JobKind>(serde_json::json!("subagent")).unwrap(),
            JobKind::SubAgent
        );
    }

    // TEST-132 (ITEM-29): a background run's status obeys the SAME terminal
    // classification as a workflow run (the backbone is one status model), and
    // `from_db_str` is `None` for an unknown value (forward-compat). The
    // late-terminal-write CAS no-op is exercised end-to-end in the DB-gated
    // repository test `mark_status`; here we pin the pure classification the CAS
    // predicate keys on (a terminal run must never be resurrected).
    #[test]
    fn status_terminal_classification_and_unknown_parse() {
        // Unknown DB status → None (in-flight, per the doc contract).
        assert_eq!(WorkflowRunStatus::from_db_str("no_such_status"), None);
        assert_eq!(WorkflowRunStatus::from_db_str("queued"), None);
        // Terminal trio the CAS guard (`status NOT IN ('cancelled','completed',
        // 'failed')`) refuses to overwrite.
        for s in [
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Cancelled,
        ] {
            assert!(s.is_terminal(), "{} must be terminal", s.as_str());
            assert!(matches!(s.as_str(), "completed" | "failed" | "cancelled"));
        }
        // Non-terminal states the guard permits transitioning.
        for s in [
            WorkflowRunStatus::Pending,
            WorkflowRunStatus::Running,
            WorkflowRunStatus::Waiting,
            WorkflowRunStatus::Resumable,
        ] {
            assert!(!s.is_terminal(), "{} must be non-terminal", s.as_str());
        }
    }
}
