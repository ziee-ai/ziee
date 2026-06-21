//! Workflow + workflow-run lifecycle events.
//!
//! Workflow CRUD: notify-and-refetch (`Workflow` / `WorkflowSystem`).
//! Workflow runs: notify-only at run.started / run.completed /
//! run.failed / run.cancelled. The per-run high-frequency progress
//! stream rides on a separate SSE endpoint (see plan §4.4 — the
//! lifecycle channel is intentionally low-frequency for cross-session
//! list views).
//!
//! B4 also adds the typed `SSEWorkflowRunEvent` enum (per-run SSE wire
//! shape) + the `ProgressEmitter` trait the runner + dispatchers emit
//! into.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};
use crate::modules::workflow::permissions::{WorkflowsManageSystem, WorkflowsRead};
use crate::modules::workflow::types::ItemProgress;

pub fn emit_user_workflow(
    action: SyncAction,
    workflow_id: Uuid,
    owner_user_id: Uuid,
    origin: Option<Uuid>,
) {
    sync_publish(
        SyncEntity::Workflow,
        action,
        workflow_id,
        Audience::owner(owner_user_id),
        origin,
    );
}

pub fn emit_system_workflow(action: SyncAction, workflow_id: Uuid, origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::WorkflowSystem,
        action,
        workflow_id,
        Audience::perm::<WorkflowsManageSystem>(),
        origin,
    );
    sync_publish(
        SyncEntity::Workflow,
        action,
        workflow_id,
        Audience::perm::<WorkflowsRead>(),
        origin,
    );
}

/// Lifecycle event for one workflow_run row (run.started /
/// run.completed / run.failed / run.cancelled). Notify-only —
/// rich progress goes through the per-run SSE channel.
pub fn emit_workflow_run(action: SyncAction, run_id: Uuid, owner_user_id: Uuid, origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::WorkflowRun,
        action,
        run_id,
        Audience::owner(owner_user_id),
        origin,
    );
}

// ============================================================
// Per-run SSE event enum (B4)
// ============================================================

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEConnectedData {
    pub message: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSESnapshotData {
    pub run_id: Uuid,
    pub status: String,
    pub current_step: Option<String>,
    pub total_tokens: u64,
    pub step_outputs_json: serde_json::Value,
    pub step_item_progress_json: serde_json::Value,
    pub step_logs_json: serde_json::Value,
    pub step_artifacts_json: serde_json::Value,
    pub pending_elicitation_json: Option<serde_json::Value>,
    pub final_output_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSERunStartedData {
    pub run_id: Uuid,
    pub workflow_id: Uuid,
    pub model_id: Option<Uuid>,
    pub sandbox_flavor: Option<String>,
    pub total_steps: u32,
    pub conversation_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEStepStartedData {
    pub run_id: Uuid,
    pub step_id: String,
    pub step_kind: String,
    pub step_index: u32,
    pub total_steps: u32,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEStepItemProgressData {
    pub run_id: Uuid,
    pub step_id: String,
    pub progress: ItemProgress,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEStepCompletedData {
    pub run_id: Uuid,
    pub step_id: String,
    pub output_preview: String,
    pub tokens_used: u64,
    pub ms_elapsed: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEStepFailedData {
    pub run_id: Uuid,
    pub step_id: String,
    pub error: String,
    pub tokens_used: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEElicitationRequiredData {
    pub run_id: Uuid,
    pub step_id: String,
    pub elicitation_id: Uuid,
    pub message: String,
    pub schema: serde_json::Value,
    /// D2: rendered seed data the FE form pre-fills with (see
    /// `StepConfig::Elicit.data`). `None` when the step declares no `data:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub deadline_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSEElicitationResolvedData {
    pub run_id: Uuid,
    pub step_id: String,
    pub elicitation_id: Uuid,
    pub resolved_by: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSERunCompletedData {
    pub run_id: Uuid,
    pub outputs_preview: serde_json::Value,
    pub total_tokens: u64,
    pub ms_elapsed: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSERunCancelledData {
    pub run_id: Uuid,
    pub cancelled_at_step: Option<String>,
    pub total_tokens: u64,
    pub tokens_at_cancel: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSERunFailedData {
    pub run_id: Uuid,
    pub error: String,
    pub total_tokens: u64,
    pub failed_at_step: Option<String>,
}

crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSEWorkflowRunEvent {
        Connected(SSEConnectedData),
        Snapshot(SSESnapshotData),
        RunStarted(SSERunStartedData),
        StepStarted(SSEStepStartedData),
        StepItemProgress(SSEStepItemProgressData),
        StepCompleted(SSEStepCompletedData),
        StepFailed(SSEStepFailedData),
        ElicitationRequired(SSEElicitationRequiredData),
        ElicitationResolved(SSEElicitationResolvedData),
        RunCompleted(SSERunCompletedData),
        RunCancelled(SSERunCancelledData),
        RunFailed(SSERunFailedData),
    }
}

impl SSEWorkflowRunEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SSEWorkflowRunEvent::RunCompleted(_)
                | SSEWorkflowRunEvent::RunCancelled(_)
                | SSEWorkflowRunEvent::RunFailed(_)
        )
    }
}

/// Abstraction the runner / dispatchers emit per-step events through.
/// `PerRunEmitter` fans out to the per-run SSE client map; tests can
/// substitute a `CapturingEmitter` that pushes into a `Vec`.
pub trait ProgressEmitter: Send + Sync {
    fn emit(&self, ev: SSEWorkflowRunEvent);
}

/// Production emitter — looks up the run's `RunHandle` in the registry
/// and fans out to every connected client's mpsc.
pub struct PerRunEmitter {
    pub run_id: Uuid,
}

impl ProgressEmitter for PerRunEmitter {
    fn emit(&self, ev: SSEWorkflowRunEvent) {
        crate::modules::workflow::registry::broadcast(self.run_id, ev);
    }
}

/// Test-only emitter (also used at the workflow_mcp progress bridge
/// site in B5 — wraps an mpsc).
pub struct ChannelEmitter {
    pub tx: tokio::sync::mpsc::UnboundedSender<SSEWorkflowRunEvent>,
}

impl ProgressEmitter for ChannelEmitter {
    fn emit(&self, ev: SSEWorkflowRunEvent) {
        let _ = self.tx.send(ev);
    }
}

#[cfg(test)]
pub struct CapturingEmitter {
    pub events: std::sync::Mutex<Vec<SSEWorkflowRunEvent>>,
}

#[cfg(test)]
impl CapturingEmitter {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }
    pub fn take(&self) -> Vec<SSEWorkflowRunEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

#[cfg(test)]
impl ProgressEmitter for CapturingEmitter {
    fn emit(&self, ev: SSEWorkflowRunEvent) {
        self.events.lock().unwrap().push(ev);
    }
}
