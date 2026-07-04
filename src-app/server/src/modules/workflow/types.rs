//! Workflow REST DTOs and runner-side types.
//!
//! REST DTOs were originally B2 (install only); B4 extends with the
//! runner-owned `RunContext`, `OutputMeta`, and per-step / per-run
//! event payloads consumed by the per-run SSE endpoint
//! (`progress_sse.rs`) and the workflow_mcp progress bridge (B5).


use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::Workflow;

// ============================================================
// Install / list DTOs (B2)
// ============================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateWorkflowFromHubRequest {
    pub hub_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSystemWorkflowFromHubRequest {
    pub hub_id: String,
    #[serde(default)]
    pub groups: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowFromHubResponse {
    pub workflow: Workflow,
    pub hub_tracking: crate::modules::hub::models::HubEntity,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowListResponse {
    pub workflows: Vec<Workflow>,
}

// ============================================================
// Run request / response (B4)
// ============================================================

/// Body of `POST /api/workflows/{id}/run`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkflowRunRequest {
    #[serde(default)]
    pub inputs: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
    /// Explicit model for a standalone (no-conversation) run, picked in the
    /// Run dialog. When set, it wins over the conversation's model and is
    /// access-checked against the user's providers (the run handler is only
    /// gated on `WorkflowsExecute`, so this is the per-model authorization).
    /// When unset, the model is snapshotted from `conversation_id` (legacy
    /// path). A run with neither is rejected (`WORKFLOW_NO_MODEL_SOURCE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<Uuid>,
    /// Opt-in: force full per-step log capture (prompt / raw_output / stderr /
    /// items) for THIS run regardless of the workflow's per-step `log:` levels,
    /// so a manually-launched run is debuggable in the run history. Off by
    /// default. The captured logs are surfaced to the run owner only.
    #[serde(default)]
    pub capture_logs: bool,
    /// Per-step canned responses, keyed by step id. ONLY honored when the
    /// workflow's `is_dev = true` (the route handler rejects mocks for
    /// published workflows with 403). Lets dev iteration + `tests/` fixtures
    /// stub specific steps without spending LLM tokens. See plan §1.
    #[serde(default)]
    pub mocks: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowRunStartResponse {
    pub run_id: Uuid,
    pub status: String,
}

/// Lightweight run-history row (A4) — excludes the heavy JSONB blobs the full
/// `WorkflowRun` carries; backs `GET /workflows/{id}/runs`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowRunSummary {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub status: String,
    /// `"manual"` (workflow page) or `"conversation"` (LLM tool call).
    pub invocation_source: String,
    pub conversation_id: Option<Uuid>,
    pub model_id: Option<Uuid>,
    pub total_tokens: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkflowRunListResponse {
    pub runs: Vec<WorkflowRunSummary>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ElicitationResponseRequest {
    pub response: serde_json::Value,
}

/// `POST /api/workflows/system/{id}/groups` body. Replaces the entire
/// set (mirrors `skill::types::SkillGroupsRequest`).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkflowGroupsRequest {
    pub group_ids: Vec<Uuid>,
}

/// `GET/PUT /api/groups/{group_id}/system-workflows` response — the system
/// workflows assigned to a group (group → workflows direction, for the User
/// Groups page widget). Mirrors MCP's `GroupSystemServersResponse`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GroupSystemWorkflowsResponse {
    pub workflows: Vec<Workflow>,
}

/// `PUT /api/groups/{group_id}/system-workflows` body — the full desired set
/// of system-workflow ids for the group. Mirrors MCP's
/// `UpdateGroupSystemServersRequest`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateGroupSystemWorkflowsRequest {
    pub workflow_ids: Vec<Uuid>,
}

// ============================================================
// Runner-side types (B4)
// ============================================================

/// Per-step output metadata. The actual content lives on disk under
/// `<workspace>/<conv>/workflow/<run>/outputs/<step_id>{.json|.txt}`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutputMeta {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub preview: String,
    pub kind: StepKindTag,
    pub parsed_as: ParsedAs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKindTag {
    Llm,
    LlmMap,
    Sandbox,
    Elicit,
    Tool,
}

impl StepKindTag {
    // Stable wire/log string for each tag; no caller yet (the enum is used via
    // its serde repr today).
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            StepKindTag::Llm => "llm",
            StepKindTag::LlmMap => "llm_map",
            StepKindTag::Sandbox => "sandbox",
            StepKindTag::Elicit => "elicit",
            StepKindTag::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParsedAs {
    Json,
    Text,
}

/// Per-step artifact metadata (post-step file collection from
/// `artifacts/<step_id>/`). Persisted into `step_artifacts_json`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactMeta {
    pub filename: String,
    pub host_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub mime_type: String,
    pub description: Option<String>,
}

/// Per-`llm_map` per-item progress snapshot (counters only — no
/// per-item content; that lives in the step's output file).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ItemProgress {
    pub completed: u32,
    pub total: u32,
    pub failed: u32,
    pub skipped: u32,
    pub tokens_so_far: u64,
}

/// Persisted under `workflow_runs.pending_elicitation_json` for the
/// duration of an elicit step's wait.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PendingElicitationRecord {
    /// The owning run. Mirrors `SSEElicitationRequiredData.run_id` so the
    /// page-reload snapshot (which deserializes this record) has the same shape
    /// as the live SSE frame. `#[serde(default)]` so an in-flight elicit row
    /// written before this field existed still deserializes (→ nil, harmless —
    /// the submit handler takes run_id from the URL path, not this record).
    #[serde(default)]
    pub run_id: Uuid,
    pub elicitation_id: Uuid,
    pub step_id: String,
    pub message: String,
    pub schema: serde_json::Value,
    /// D2: rendered seed data for the form (see `StepConfig::Elicit.data`).
    /// `None` when the step declares no `data:`. Rides the existing
    /// `pending_elicitation_json` JSONB column — no migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub deadline_at: DateTime<Utc>,
}

/// In-memory runner-owned state — never persisted directly. The
/// per-step persistence path is `repository::persist_step_metadata`.
#[derive(Debug)]
pub struct RunContext {
    pub run_id: Uuid,
    pub user_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub workflow_id: Uuid,
    pub inputs: HashMap<String, serde_json::Value>,
    pub step_outputs: HashMap<String, OutputMeta>,
    pub step_item_progress: HashMap<String, ItemProgress>,
    pub extracted_path: PathBuf,
    /// `<workspace_root>/<conv_id>/workflow/<run_id>/`. The staged
    /// dir lives here for the run's lifetime; cleaned on terminal.
    pub sandbox_workspace: PathBuf,
    pub outputs_dir: PathBuf,
    /// `<sandbox_workspace>/artifacts/`. Per-step subdirs are created
    /// on demand by the SandboxDispatcher.
    pub artifacts_dir: PathBuf,
    /// `<sandbox_workspace>/inputs/`. The SandboxDispatcher writes
    /// per-step stdin files here.
    pub inputs_dir: PathBuf,
    /// Resolved at run start from the conversation's snapshotted
    /// `model_id` — passed as `ChatRequest.model` for every `llm`
    /// step.
    pub model_id: Uuid,
    pub model_name: String,
    /// Request `max_tokens` for every llm/llm_map call — the model's
    /// configured `parameters.max_tokens` (fallback 8192), matching the chat
    /// path. NOT the per-call cost cap: hardcoding 50k here exceeds many
    /// models' output limits (e.g. Claude Opus 4.1 = 32k) and the provider
    /// rejects the request. The cost cap is enforced post-call by the
    /// accumulator.
    pub model_max_tokens: u32,
    pub sandbox_flavor: Option<String>,
    pub total_tokens: u64,
    /// Cumulative bytes of step OUTPUT files + collected ARTIFACT files
    /// across the whole run. The runner enforces the per-run 100 MiB cap
    /// against this after each step (plan §4.5 + §10 / audit gap 6).
    pub total_output_bytes: u64,
    pub is_dev: bool,
    /// Runtime per-step mocks from the `/run` request body. ONLY populated
    /// when `is_dev` (the handler drops them otherwise). The runner short-
    /// circuits a step whose id is in this map (or whose `StepDef.mock` is
    /// set) by writing the canned value as the step output instead of
    /// dispatching. See plan §1 + the B4 audit MAJOR finding.
    pub mocks: HashMap<String, serde_json::Value>,
    /// Test-run mock exception (B6). `POST /api/workflows/{id}/test` runs
    /// bundled `tests/*.yaml` fixtures with their mocks — a sanctioned mock
    /// context. When `true`, the runner honors `mocks` (and `StepDef.mock`)
    /// EVEN on a published (`is_dev = false`) workflow. This does NOT relax
    /// the `/run` endpoint's is_dev gate: only the test handler sets it.
    /// See plan §3 (`/test` "tests/ files providing mocks still run them in
    /// test-mode without spending tokens").
    pub force_mocks: bool,
    /// When `true`, the runner persists this run's declared `expose: artifact`
    /// outputs / collected artifacts + tool-result files to the user file store
    /// on completion (durable + visible in Files). Set on the REST `/run` path;
    /// `false` on the `workflow_mcp` tool-call path (the chat extension persists
    /// those instead). See A3.
    pub persist_artifacts: bool,
    /// When `true`, every step captures full logs (prompt/raw_output/stderr/
    /// items) regardless of its declared `log:` level — the per-run "Capture
    /// debug logs" toggle. See A7.
    pub force_log_capture: bool,
    /// E7: running total of durable log-body bytes stored in `step_logs_json`
    /// across the whole run. The per-log cap (`LOG_BODY_CAP_CHARS`) bounds each
    /// body; this aggregate cap (`RUN_LOG_BODY_CAP_CHARS`) bounds the run so a
    /// many-step debug-capture run can't bloat the row. `AtomicU64` so the
    /// immutable-`&RunContext` log_io writers can bump it.
    pub total_log_bytes: std::sync::atomic::AtomicU64,
}

impl RunContext {
    pub fn step_output_host_path(&self, step_id: &str, parsed_as: ParsedAs) -> PathBuf {
        let ext = match parsed_as {
            ParsedAs::Json => "json",
            ParsedAs::Text => "txt",
        };
        self.outputs_dir.join(format!("{step_id}.{ext}"))
    }

    pub fn step_output_sandbox_path(&self, step_id: &str) -> String {
        // The sandbox sees the run dir mounted at /home/sandboxuser/workflow/<run>/.
        // Outputs are at outputs/<step_id>{.json|.txt} from the run-dir CWD.
        let meta = self.step_outputs.get(step_id);
        let parsed_as = meta.map(|m| m.parsed_as).unwrap_or(ParsedAs::Text);
        let ext = match parsed_as {
            ParsedAs::Json => "json",
            ParsedAs::Text => "txt",
        };
        format!("outputs/{step_id}.{ext}")
    }

    pub fn artifact_path_for_step(&self, step_id: &str) -> PathBuf {
        self.artifacts_dir.join(step_id)
    }

    pub fn sandbox_run_dir_str(&self) -> String {
        format!("/home/sandboxuser/workflow/{}", self.run_id)
    }
}

/// Per-step dispatcher result.
#[derive(Debug)]
pub enum StepResult {
    Completed {
        output: serde_json::Value,
        parsed_as: ParsedAs,
        tokens_used: u64,
        ms_elapsed: u64,
    },
    Failed {
        error: String,
        tokens_used: u64,
    },
    Cancelled,
    /// Durable resume: the step parked the run on an indefinite (`timeout_ms:
    /// 0`) human `elicit` gate. The runner exits to `waiting` WITHOUT marking a
    /// terminal status; it re-spawns (resume_run) when the user submits. No
    /// output is written until the response is consumed on resume.
    Suspended,
}
