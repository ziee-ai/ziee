//! Shared agent-activity mapping + a persist-only `EventSink` (ITEM-1).
//!
//! The workflow `kind: agent` step, detached background runs, and fan-out child
//! sub-agents ALL surface the SAME `AgentActivity` entry stream. This module is
//! the single place the `agent_core::AgentEvent` → `ProgressKind::AgentActivity`
//! mapping lives, so `WorkflowEventSink` (live SSE + durable) and
//! [`PersistingActivitySink`] (durable-only — used by background runs and fan-out
//! children, which have no attached live SSE track) can never drift.

use std::sync::atomic::{AtomicU64, Ordering};

use agent_core::{AgentEvent, EventSink};
use ai_providers::{ContentBlock, Role};
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use super::events::{AgentActivityKind, AgentActivityStatus, ProgressKind};
use super::repository;

/// Per-entry byte caps applied before an activity is emitted / persisted (so a
/// runaway thought or tool blob can't bloat the SSE frame or the durable row).
pub(crate) const AGENT_ACTIVITY_TITLE_MAX_BYTES: usize = 512;
pub(crate) const AGENT_ACTIVITY_DETAIL_MAX_BYTES: usize = 16 * 1024;

/// Truncate `s` to at most `max` bytes on a UTF-8 char boundary.
pub(crate) fn truncate_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// One mapped unit of an `AgentEvent`: either a STRUCTURED activity entry (which
/// both accumulates on its own SSE track AND persists durably) or a rolled-up
/// progress LOG line (SSE-only; NOT part of the durable transcript). This split
/// is exactly the two shapes `WorkflowEventSink` produces today.
pub(crate) enum MappedActivity {
    Activity {
        kind: AgentActivityKind,
        tool: Option<String>,
        title: String,
        detail: Option<String>,
        status: AgentActivityStatus,
    },
    Log {
        line: String,
    },
}

/// The single `AgentEvent` → activity mapping, reused by every host sink. Order
/// is significant: a `Message` yields one entry per thinking/tool block plus a
/// trailing assistant-text entry, in that order, so seq assignment is stable.
pub(crate) fn map_agent_event(ev: &AgentEvent) -> Vec<MappedActivity> {
    let mut out = Vec::new();
    match ev {
        AgentEvent::Message(msg) => {
            // Surface each thinking block + tool request as its own entry, plus a
            // short assistant-text preview.
            for b in &msg.content {
                match b {
                    ContentBlock::Thinking { thinking, .. } => out.push(MappedActivity::Activity {
                        kind: AgentActivityKind::Thinking,
                        tool: None,
                        title: thinking.chars().take(200).collect::<String>(),
                        detail: Some(thinking.clone()),
                        status: AgentActivityStatus::Ok,
                    }),
                    ContentBlock::ToolUse { name, input, .. } => {
                        out.push(MappedActivity::Activity {
                            kind: AgentActivityKind::ToolCall,
                            tool: Some(name.clone()),
                            title: format!("→ {name}"),
                            detail: serde_json::to_string(input).ok(),
                            status: AgentActivityStatus::Running,
                        })
                    }
                    _ => {}
                }
            }
            if msg.role == Role::Assistant {
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    out.push(MappedActivity::Activity {
                        kind: AgentActivityKind::Message,
                        tool: None,
                        title: text.chars().take(200).collect::<String>(),
                        detail: Some(text),
                        status: AgentActivityStatus::Ok,
                    });
                }
            }
        }
        AgentEvent::ToolNotification { server, note } => out.push(MappedActivity::Activity {
            kind: AgentActivityKind::ToolResult,
            tool: Some(server.clone()),
            title: format!("{server}: {note}"),
            detail: Some(note.clone()),
            status: AgentActivityStatus::Ok,
        }),
        AgentEvent::GateOpened(_) => out.push(MappedActivity::Activity {
            kind: AgentActivityKind::Gate,
            tool: None,
            title: "awaiting human input".to_string(),
            detail: None,
            status: AgentActivityStatus::Running,
        }),
        AgentEvent::HistoryReplaced { summary_upto } => out.push(MappedActivity::Activity {
            kind: AgentActivityKind::Compaction,
            tool: None,
            title: format!("context compacted ({summary_upto} messages summarized)"),
            detail: None,
            status: AgentActivityStatus::Ok,
        }),
        // The agent's task list changed — rolled up to ONE compact log line (the
        // workflow run has no dedicated checklist surface).
        AgentEvent::TaskListChanged { items, .. } => {
            let total = items.len();
            let completed = items
                .iter()
                .filter(|t| t.status == agent_core::TaskStatus::Completed)
                .count();
            let line = match items
                .iter()
                .find(|t| t.status == agent_core::TaskStatus::InProgress)
            {
                Some(active) => format!("tasks: {completed}/{total} — {}", active.active_form),
                None => format!("tasks: {completed}/{total}"),
            };
            out.push(MappedActivity::Log { line });
        }
        // A `delegate` fan-out's per-child status changed — rolled up to ONE
        // compact log line (the sub-agent card is the CHAT host's surface).
        AgentEvent::SubAgentActivity { children, .. } => {
            let total = children.len();
            let settled = children
                .iter()
                .filter(|c| {
                    matches!(
                        c.status,
                        agent_core::SubAgentChildStatus::Completed
                            | agent_core::SubAgentChildStatus::Failed
                    )
                })
                .count();
            let failed = children
                .iter()
                .filter(|c| c.status == agent_core::SubAgentChildStatus::Failed)
                .count();
            let line = if failed > 0 {
                format!("sub-agents: {settled}/{total} settled ({failed} failed)")
            } else {
                format!("sub-agents: {settled}/{total} settled")
            };
            out.push(MappedActivity::Log { line });
        }
        // Live token stream / usage / stop — no durable activity entry.
        AgentEvent::ContentDelta(_) | AgentEvent::Usage(_) | AgentEvent::Stopped(_) => {}
    }
    out
}

/// A persist-ONLY `EventSink`: maps each `AgentEvent` to durable
/// `ProgressKind::AgentActivity` rows on `workflow_runs.step_logs_json` (keyed
/// `"{step_id}::agent_activity"`), with NO live SSE emission. Used where the run
/// has no attached stream — a DETACHED background run (its own run row) and a
/// fan-out CHILD (its own `subagent` run row). Rolled-up `Log` lines are dropped
/// (they are ephemeral SSE progress, never part of the transcript).
///
/// The append is AWAITED (not fire-and-forget): the `seq` is assigned before the
/// await so ordering is preserved, and awaiting guarantees every activity is
/// persisted before the caller marks the run terminal (`settle_child`) — no race.
/// A DB error is logged and swallowed; it must never fail the agent loop.
pub struct PersistingActivitySink {
    pool: PgPool,
    run_id: Uuid,
    step_id: String,
    seq: AtomicU64,
}

impl PersistingActivitySink {
    pub fn new(pool: PgPool, run_id: Uuid, step_id: impl Into<String>) -> Self {
        Self {
            pool,
            run_id,
            step_id: step_id.into(),
            seq: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl EventSink for PersistingActivitySink {
    async fn emit(&self, ev: AgentEvent) {
        for mapped in map_agent_event(&ev) {
            let MappedActivity::Activity {
                kind,
                tool,
                title,
                detail,
                status,
            } = mapped
            else {
                continue; // Log-kind rollups are SSE-only; never persisted.
            };
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let activity = ProgressKind::AgentActivity {
                seq,
                kind,
                tool,
                title: truncate_bytes(title, AGENT_ACTIVITY_TITLE_MAX_BYTES),
                detail: detail.map(|d| truncate_bytes(d, AGENT_ACTIVITY_DETAIL_MAX_BYTES)),
                status,
            };
            match serde_json::to_value(&activity) {
                Ok(entry) => {
                    if let Err(e) = repository::append_agent_activity(
                        &self.pool,
                        self.run_id,
                        &self.step_id,
                        &entry,
                    )
                    .await
                    {
                        tracing::warn!("agent activity persist failed: {e}");
                    }
                }
                Err(e) => tracing::warn!("agent activity serialize failed: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{SubAgentChild, SubAgentChildStatus};
    use ai_providers::ChatMessage;

    fn kinds(mapped: &[MappedActivity]) -> Vec<&'static str> {
        mapped
            .iter()
            .map(|m| match m {
                MappedActivity::Activity { .. } => "activity",
                MappedActivity::Log { .. } => "log",
            })
            .collect()
    }

    /// TEST-6: a `Message` with a thinking block + a tool call + assistant text
    /// maps to three ORDERED activity entries (thinking, tool_call, message).
    #[test]
    fn message_maps_to_ordered_activity_entries() {
        let msg = ChatMessage::with_blocks(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    thinking: "let me think".into(),
                    signature: None,
                },
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "search".into(),
                    input: serde_json::json!({ "q": "x" }),
                },
                ContentBlock::Text {
                    text: "the answer".into(),
                },
            ],
        );
        let mapped = map_agent_event(&AgentEvent::Message(msg));
        assert_eq!(kinds(&mapped), vec!["activity", "activity", "activity"]);
        match &mapped[0] {
            MappedActivity::Activity { kind, .. } => {
                assert_eq!(*kind, AgentActivityKind::Thinking)
            }
            _ => panic!("expected thinking"),
        }
        match &mapped[1] {
            MappedActivity::Activity { kind, tool, .. } => {
                assert_eq!(*kind, AgentActivityKind::ToolCall);
                assert_eq!(tool.as_deref(), Some("search"));
            }
            _ => panic!("expected tool_call"),
        }
        match &mapped[2] {
            MappedActivity::Activity { kind, title, .. } => {
                assert_eq!(*kind, AgentActivityKind::Message);
                assert_eq!(title, "the answer");
            }
            _ => panic!("expected message"),
        }
    }

    /// TEST-6: TaskListChanged / SubAgentActivity roll up to a LOG line (never a
    /// persisted activity entry); ContentDelta/Usage/Stopped map to nothing.
    #[test]
    fn rollups_and_streamed_events_are_log_or_empty() {
        let sub = AgentEvent::SubAgentActivity {
            run_id: Uuid::nil(),
            children: vec![SubAgentChild {
                id: "c".into(),
                label: "L".into(),
                status: SubAgentChildStatus::Completed,
            }],
        };
        assert_eq!(kinds(&map_agent_event(&sub)), vec!["log"]);
        assert!(
            map_agent_event(&AgentEvent::Stopped(agent_core::StopReason::NoToolCall)).is_empty()
        );
    }
}
