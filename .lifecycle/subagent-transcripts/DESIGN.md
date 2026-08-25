# Design: Surface agent transcripts for background runs and fan-out children

## Problem

Two agent kinds have **no after-the-fact transcript** for the user:

- **Background runs** (`modules/background_mcp`) — the loop's `BackgroundEventSink::emit`
  is a literal no-op (`tools.rs:293-298`, its own doc calls surfacing "a follow-up").
  The user sees only final result + status.
- **Fan-out child sub-agents** (`agent-core/fanout.rs`) — each child runs on a
  `NoopEventSink` (`fanout.rs:275`) + in-memory `EphemeralTranscript`; only a
  neutralized summary returns to the parent. The chat `SubAgentActivityCard` shows
  live, status-only rows that are dropped on reload.

Meanwhile the workflow `kind: agent` step already has a **complete, reusable**
persisted activity-timeline stack: `append_agent_activity` (repository.rs:826) →
`step_logs_json["<stepId>::agent_activity"]` → `ProgressKind::AgentActivity`
(events.rs:256) → the `AgentActivityTimeline` React component
(rehydrated from `step_logs_json` in `workflowRun/actions/subscribe.ts:52`).

## Goal (from the owner)

Surface transcripts for BOTH kinds. **Full per-child transcript** for fan-out
children (thinking + tool calls/results + messages, not just a result). Ship
**both in one feature**.

## Core strategy: REUSE the workflow agent-activity timeline for both

Both halves persist the SAME `AgentActivity` entry stream the workflow host
already produces, and render it with the SAME `AgentActivityTimeline` component.
No new timeline format, no new render component.

### Half A — Background runs (mostly wiring)

A background run is ALREADY its own `workflow_runs` row (`insert_background_run`,
runner.rs). So:

1. Replace the no-op `BackgroundEventSink` with a **persisting sink** that mirrors
   `WorkflowEventSink::emit`'s `AgentEvent → append_agent_activity` mapping
   (agent_dispatch.rs:432). Keyed by the background run's own id + a fixed step id
   (e.g. `"agent"`). No live SSE track needed (no attached stream) — persist only.
2. Project `step_logs_json -> '<step>::agent_activity'` in `get_background_run_detail`
   (repository.rs:1867) and add an `activity: Vec<AgentActivityEntry>` field to
   `BackgroundRunDetail` (types.rs:153). Owner-scoped already (WHERE user_id).
3. Render `<AgentActivityTimeline>` inside `BackgroundRunResult` / `BackgroundRunCard`.

Zero schema change. Zero new component.

### Half B — Fan-out children (the architectural half)

**Model each fan-out child as its OWN `workflow_runs` row** (`job_kind='subagent'`),
so a child inherits the entire stack: activity persistence, the read pattern,
retention, and `ON DELETE CASCADE`. This needs ONE new nullable column
`parent_run_id UUID REFERENCES workflow_runs(id) ON DELETE CASCADE` (a child is
deleted with its parent) — migration prefix > `202608210100`.

The child run_id is minted inside agent-core (`fanout.rs:282`), which is
domain-free/DB-free. So the host injects a **child-sink factory port**:

- **agent-core seam** (`fanout.rs:273-279`, the `isolate_children` block): add an
  optional `child_sink_factory: Option<Arc<dyn ChildSink>>` field on `AgentCore`.
  When present, the child's `NoopEventSink` is replaced by
  `factory.for_child(parent_run_id, child_run_id, label)`. Absent ⇒ unchanged
  (Noop). agent-core stays DB-free — it only calls an injected port (mirrors the
  existing `TranscriptStore`/`EventSink` port pattern).
- **server factory**: `for_child(...)` creates the child `workflow_runs` row
  (`parent_run_id` = the parent, `conversation_id` carried through) and returns a
  persisting sink keyed to the child run — reusing `append_agent_activity`.
- **The chat parent needs a run identity to be the `parent_run_id`.** The chat host
  turn is NOT a workflow_runs row today. Rather than make chat turns into run rows,
  the child links via the SAME key the `SubAgentActivityCard` already uses — the
  parent **assistant `message_id`** + `conversation_id`. So the child row carries
  `parent_message_id` (nullable) for chat parents and/or `parent_run_id` for
  workflow/background parents. (Resolved in DECISIONS — one linkage per host.)

### Read + frontend for children

- Owner-scoped `GET` returning a parent's children (each: id, label, status,
  activity[]). For chat: keyed by parent assistant message_id; for a run parent:
  by parent_run_id.
- Extend `SubAgentActivityCard` from status-only to a drill-in: each child row
  expands to its `<AgentActivityTimeline>`, fetched on demand (lazy, like
  `BackgroundRunCard.toggleResult`). Live status still comes from the SSE frame;
  the persisted transcript is the after-the-fact source that survives reload.

## Invariants (the promises)

- **INV-1**: A user can view, AFTER completion, the step-by-step activity
  (thinking, tool calls/results, messages) of a **background run they own**,
  rendered by the workflow `AgentActivityTimeline`.
- **INV-2**: A user can view, after the run, the **full per-child transcript**
  (thinking + tool calls/results + messages) of each fan-out child sub-agent their
  run spawned.
- **INV-3**: The fan-out security boundary is preserved — a child's raw output
  still returns to the PARENT LLM only as a neutralized summary
  (`summary_from_events`, fanout.rs:303); user-visible persistence NEVER re-enters
  the parent's context.
- **INV-4**: All transcript reads are **owner-scoped** — a foreign run/child id
  yields 404, matching `get_background_run_detail`'s existing `WHERE user_id`.
- **INV-5**: `agent-core` stays domain-free and DB-free — child-activity
  persistence is via an injected port, never a DB/schema dependency in the crate.

## Security note (INV-3 in full)

The isolation at `fanout.rs:273-279` exists so a child's untrusted output can't act
as instructions to the PARENT LLM (prompt-injection defense). This feature persists
child activity for the **USER** to view; it does NOT change what returns to the
parent model (still summary-only). Persisting-for-display ≠ feeding-back-to-parent,
so the boundary is intact. An acceptance test asserts the parent still receives only
the neutralized summary.

## Non-goals

- Not surfacing the raw provider `agent_transcript_json` resume blob (that stays
  resume-only). The `AgentActivity` timeline is the user-facing transcript.
- Not changing the parent-LLM summary boundary.
- Not a live-streaming rework of background runs (persist-and-read; the SSE live
  path for chat fan-out stays as-is for the live card).

## Patterns to mirror

- Persistence + read: workflow `kind: agent` (`agent_dispatch.rs` WorkflowEventSink,
  `repository.rs::append_agent_activity` / `get_background_run_detail`).
- agent-core port: the existing `EventSink`/`TranscriptStore` injected-port pattern
  (`ports.rs`).
- Frontend: `AgentActivityTimeline` + `WorkflowRunProgressView` rehydration;
  `BackgroundRunCard.toggleResult` lazy-load idiom.
- Migration: an FK `... REFERENCES workflow_runs(id) ON DELETE CASCADE` (mirrors
  `background_run_notes` 202607191200).
