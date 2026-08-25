//! ITEM-7 — the CHAT host's fan-out child-sink factory.
//!
//! The agent-core `ChildSink` seam is domain-free: it hands back only a child
//! `run_id` + label. This concrete factory bakes in the parent turn's identity
//! (owner / conversation / assistant `message_id` / model) at construction, and on
//! `for_child` it:
//!   1. inserts a `workflow_runs` row for the child (`job_kind='subagent'`, linked
//!      to the parent message so it cascade-deletes with the conversation), then
//!   2. returns a [`PersistingActivitySink`] keyed to that child run id, so the
//!      child's OWN transcript (thinking / tool activity / messages) is persisted
//!      for later user display — never fed back to the parent (INV-3).
//! On `settle_child` it marks the child row terminal + emits an owner-scoped
//! `WorkflowRun` sync (ITEM-10) so an open drill-in refetches.
//!
//! A child-row insert failure DEGRADES gracefully: a `PersistingActivitySink` is
//! still returned, but its `append`/`set_run_status` UPDATEs no-op on the missing
//! row — the child still RUNS, it just has no persisted transcript.

use std::sync::Arc;

use agent_core::{ChildSink, EventSink};
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::sync::SyncAction;
use crate::modules::workflow::activity_sink::PersistingActivitySink;
use crate::modules::workflow::events::emit_workflow_run;
use crate::modules::workflow::repository as wf_repo;

pub struct ChatChildSinkFactory {
    pool: PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    parent_message_id: Uuid,
    model_id: Option<Uuid>,
}

impl ChatChildSinkFactory {
    pub fn new(
        pool: PgPool,
        user_id: Uuid,
        conversation_id: Option<Uuid>,
        parent_message_id: Uuid,
        model_id: Option<Uuid>,
    ) -> Self {
        Self {
            pool,
            user_id,
            conversation_id,
            parent_message_id,
            model_id,
        }
    }
}

#[async_trait]
impl ChildSink for ChatChildSinkFactory {
    async fn for_child(&self, child_run_id: Uuid, label: &str) -> Arc<dyn EventSink> {
        match wf_repo::insert_subagent_child_run(
            &self.pool,
            child_run_id,
            self.parent_message_id,
            self.conversation_id,
            self.user_id,
            self.model_id,
            label,
        )
        .await
        {
            Ok(()) => {
                // Owner-scoped: the new child appears on a refetch (ITEM-10).
                emit_workflow_run(SyncAction::Create, child_run_id, self.user_id, None);
            }
            Err(e) => tracing::warn!("subagent child run insert failed: {e}"),
        }
        Arc::new(PersistingActivitySink::new(
            self.pool.clone(),
            child_run_id,
            "agent",
        ))
    }

    async fn settle_child(&self, child_run_id: Uuid, ok: bool) {
        let status = if ok { "completed" } else { "failed" };
        if let Err(e) = wf_repo::set_run_status(&self.pool, child_run_id, status).await {
            tracing::warn!("subagent child settle failed: {e}");
        }
        emit_workflow_run(SyncAction::Update, child_run_id, self.user_id, None);
    }
}
