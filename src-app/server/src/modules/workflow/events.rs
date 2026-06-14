//! Workflow + workflow-run lifecycle events.
//!
//! Workflow CRUD: notify-and-refetch (`Workflow` / `WorkflowSystem`).
//! Workflow runs: notify-only at run.started / run.completed /
//! run.failed / run.cancelled. The per-run high-frequency progress
//! stream rides on a separate SSE endpoint (see plan §4.4 — the
//! lifecycle channel is intentionally low-frequency for cross-session
//! list views).

#![allow(dead_code)]

use uuid::Uuid;

use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};
use crate::modules::workflow::permissions::{WorkflowsManageSystem, WorkflowsRead};

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
