# PLAN — Surface agent transcripts for background runs and fan-out children

## Design source

Realizes `.lifecycle/subagent-transcripts/DESIGN.md` (this feature's design doc) in
full — Half A (background runs) §"Half A", Half B (fan-out children) §"Half B", and
its `## Invariants`. The design reuses the workflow `kind: agent` activity-timeline
stack (`modules/workflow`: `append_agent_activity`, `ProgressKind::AgentActivity`,
`AgentActivityTimeline.tsx`) rather than inventing a new transcript surface.

## Invariants

- **INV-1**: A user can view, AFTER completion, the step-by-step activity (thinking, tool calls/results, messages) of a background run they own, rendered by the workflow `AgentActivityTimeline`.
- **INV-2**: A user can view, after the run, the full per-child transcript (thinking + tool calls/results + messages) of each fan-out child sub-agent their run spawned.
- **INV-3**: The fan-out security boundary is preserved — a child's raw output still returns to the PARENT LLM only as a neutralized summary; user-visible persistence never re-enters the parent's context.
- **INV-4**: All transcript reads are owner-scoped — a foreign run/child id yields 404, matching `get_background_run_detail`'s existing `WHERE user_id`.
- **INV-5**: `agent-core` stays domain-free and DB-free — child-activity persistence is via an injected port, never a DB/schema dependency in the crate.

## Items

- **ITEM-1**: Extract the reusable `AgentEvent → append_agent_activity` mapping out of `WorkflowEventSink::emit` into a shared `PersistingActivitySink { pool, run_id, step_id, seq }` (server-side, `EventSink` impl, persist-only, no SSE). `WorkflowEventSink` keeps its live-SSE behavior by delegating the persistence to it. This is the single mapping both new sinks reuse.
- **ITEM-2**: Replace the no-op `BackgroundEventSink` (`background_mcp/tools.rs:293`) with a `PersistingActivitySink` keyed to the background run's own id + step id `"agent"`. Background loop activity now persists to `workflow_runs.step_logs_json["agent::agent_activity"]`.
- **ITEM-3**: Add `activity: Vec<AgentActivityEntry>` to `BackgroundRunDetail` (`workflow/types.rs:153`) and project `step_logs_json -> 'agent::agent_activity'` in `get_background_run_detail` (`repository.rs:1867`). Owner scope unchanged (`WHERE user_id`). Regen openapi + types.
- **ITEM-4**: Frontend — render `<AgentActivityTimeline stepId="agent" entries={detail.activity}>` inside `BackgroundRunResult` (shown via `BackgroundRunCard.toggleResult`). Empty activity → no timeline block (graceful).
- **ITEM-5**: agent-core seam — add `pub child_sink_factory: Option<Arc<dyn ChildSink>>` on `AgentCore` (core.rs) + the `ChildSink` trait in `ports.rs` (`fn for_child(&self, parent: ChildParentRef, child_run_id: Uuid, label: &str) -> Arc<dyn EventSink>`). In `fanout.rs:273-279` (isolate_children), replace `NoopEventSink` with the factory's sink WHEN a factory is present; absent ⇒ Noop (unchanged). Crate stays DB-free. `ChildParentRef` is an opaque host-supplied token (agent-core does not interpret it).
- **ITEM-6**: Migration (prefix > `202608210100`) — add nullable `parent_run_id UUID REFERENCES workflow_runs(id) ON DELETE CASCADE` and nullable `parent_message_id UUID` + `parent_conversation_id UUID` to `workflow_runs`, indexed for the children lookups. A child row is deleted with its parent run (cascade); chat-parented children are cleaned via `parent_conversation_id`/message linkage (see DEC).
- **ITEM-7**: Server child-sink factory — implement `ChildSink::for_child` to (a) `insert_subagent_child_run` (a `workflow_runs` row, `job_kind='subagent'`, parent linkage + `conversation_id`, owner = parent's user), then (b) return a `PersistingActivitySink` keyed to that child run id. Mark the child run terminal (completed/failed) when the child settles. Wire the CHAT host (`chat/agent_host/dispatcher.rs:221`, `isolate_children=true`) to inject the factory with the parent's `(conversation_id, assistant message_id)`; workflow/background hosts inject it with `parent_run_id`.
- **ITEM-8**: Server REST — owner-scoped `GET /api/subagent-runs?parent_message_id=&parent_run_id=` (list children: id, label, status, created_at) + `GET /api/subagent-runs/{child_id}` (detail incl. `activity[]`). Foreign id → 404. Gate on the same read perm the parent surface uses (chat: conversation ownership; run: `WorkflowsRead`).
- **ITEM-9**: Frontend — extend `SubAgentActivityCard`: each child row gains a lazy expand → fetch child detail → `<AgentActivityTimeline>`. Add `SubAgentActivity.store` actions `loadChildTranscript(childId)` + `childDetailsById`. Live status still from SSE frames; persisted transcript survives reload (the durability fix). "Showing N of M" not needed (children bounded by `SubagentLimits.max_children_per_call`=8).
- **ITEM-10**: Sync — child run terminal transition emits owner-scoped `SyncEntity::WorkflowRun` (reuse existing `emit_workflow_run`); the card self-gates its refetch on conversation ownership. Background run activity is already covered by the existing `WorkflowRun` sync on the run row.
- **ITEM-11**: [acceptance-support] A `NoopChildSink`-absent path test — with no factory injected (workflow host today, and any non-chat caller), fan-out behavior is byte-identical to main (children still Noop, summary-only to parent). Guards INV-3/INV-5 against regression.

## Files to touch

- `src-app/agent-core/src/ports.rs` — `ChildSink` trait + `ChildParentRef`.
- `src-app/agent-core/src/core.rs` — `child_sink_factory` field (+ struct-literal construction sites; `real_llm_loop.rs` test).
- `src-app/agent-core/src/fanout.rs` — inject factory sink in the isolate block.
- `src-app/server/src/modules/workflow/agent_dispatch.rs` — extract `PersistingActivitySink`; `WorkflowEventSink` delegates.
- `src-app/server/src/modules/workflow/repository.rs` — `get_background_run_detail` activity projection; `insert_subagent_child_run`; children list/detail queries.
- `src-app/server/src/modules/workflow/types.rs` — `BackgroundRunDetail.activity`; child DTOs.
- `src-app/server/src/modules/workflow/routes.rs` + `handlers/mod.rs` — subagent-runs endpoints.
- `src-app/server/src/modules/workflow/migrations/2026XXXXXXXX_subagent_child_runs.sql` — parent linkage columns + indexes.
- `src-app/server/src/modules/background_mcp/tools.rs` — persisting `BackgroundEventSink`.
- `src-app/server/src/modules/chat/agent_host/dispatcher.rs` — inject the child-sink factory.
- `src-app/ui/src/modules/background/components/{BackgroundRunCard,BackgroundRunResult}.tsx` — timeline render.
- `src-app/ui/src/modules/chat/components/agent-activity/SubAgentActivityCard.tsx` + `chat/extensions/sub-agent-activity/SubAgentActivity.store.ts` — drill-in + loader.
- `src-app/ui/src/api-client/*` + `openapi/openapi.json` — regen (both binaries).

## Patterns to follow

- **Persisting sink + read**: mirror `workflow/agent_dispatch.rs::WorkflowEventSink` + `repository.rs::append_agent_activity`/`get_background_run_detail`.
- **agent-core port**: mirror the existing `EventSink`/`TranscriptStore` injected-port pattern in `agent-core/src/ports.rs`; the crate never imports server/DB types.
- **Migration**: mirror `background_run_notes` (202607191200) FK `REFERENCES workflow_runs(id) ON DELETE CASCADE`.
- **Child DTO + endpoint**: mirror `BackgroundRunDetail` + `get_background_run` (owner-scoped, foreign → 404).
- **Frontend timeline**: reuse `AgentActivityTimeline` + the `BackgroundRunCard.toggleResult` lazy-load idiom + the `WorkflowRunProgressView` feed shape.

## UI-surface checklist (per the surfaces this feature adds/edits)

- **Precedent** — the child drill-in is the twin of `WorkflowRunProgressView`'s per-step `AgentActivityTimeline`; the background timeline is the same component in `BackgroundRunResult`. Mirror both; add no bespoke timeline chrome.
- **Scale / cardinality** — fan-out children are hard-bounded by `SubagentLimits.max_children_per_call` (default 8); background activity is capped at `AGENT_ACTIVITY_MAX_ENTRIES`=500 server-side (trimmed in `append_agent_activity`). No unbounded list; no paging needed. Timeline entries already seq-ordered + trimmed.
- **Device size** — `AgentActivityTimeline` is an existing responsive component; the drill-in inherits the card's width. Gallery coverage adds a populated timeline state at 390px.
- **Populated render** — gallery cell renders the child card EXPANDED with a representative multi-entry timeline (thinking + tool + message + error), reviewed at each viewport.
- **User-visible progress** — live status stays on the SSE card; the persisted transcript is the after-the-fact view. Status glyphs unchanged.
- **JTBD** — "after my agent spawned sub-agents / ran something in the background, I want to read exactly what each one thought and did, even after a reload, to trust/debug the result." Surfaces: background card (result + transcript), chat sub-agent card (per-child expand), empty/error (no-activity → graceful empty), mobile (390px timeline).
- **Entity-lifecycle** — a child run is deleted with its parent (cascade); the card must handle a child whose transcript 404s (parent/child pruned) → show the live status only, no crash. Covered by an infra-walk item in phase 5.
