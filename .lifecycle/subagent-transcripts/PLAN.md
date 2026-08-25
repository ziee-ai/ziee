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

## Plan audit (phase 2 — verified against the codebase)

Verified against the tree at HEAD. `verdict ∈ PASS | CONCERN | BLOCKED`.

- **ITEM-1** — verdict: PASS — `WorkflowEventSink::emit` (agent_dispatch.rs:433-573) + `push_activity`/`push_line` (agent_dispatch.rs:370-429) are the exact mapping to extract; `append_agent_activity` (repository.rs:826, key `"{step_id}::agent_activity"`, trims to `AGENT_ACTIVITY_MAX_ENTRIES`) is the persist tail. Caps `AGENT_ACTIVITY_TITLE_MAX_BYTES=512`/`DETAIL=16*1024` (agent_dispatch.rs:337-338) reused. Extraction into a shared `map_agent_event` + `PersistingActivitySink` keeps `WorkflowEventSink` byte-identical (existing TEST-7 at agent_dispatch.rs:1432 pins the mapping).
- **ITEM-2** — verdict: PASS — no-op sink confirmed at `background_mcp/tools.rs:727-732` (`struct BackgroundEventSink; impl EventSink { async fn emit(&self, _ev){} }`), injected at tools.rs:1116 into `build_detached_agent_core`. A background run IS its own `workflow_runs` row (`drive_subagent_turn` runs on `run_id`), so keying `PersistingActivitySink` to that `run_id` + step `"agent"` needs zero schema change.
- **ITEM-3** — verdict: CONCERN — `get_background_run_detail` (repository.rs:1867) uses `query_as!(BackgroundRunDetail, …)`, which requires every struct field to be a SELECT column; adding `activity` cannot be a bare struct field. Resolved in DEC: switch to `sqlx::query!` + manual struct build, projecting `coalesce(step_logs_json -> 'agent::agent_activity','[]'::jsonb)` and deserializing to `Vec<ProgressKind>` (the FE `AgentActivityEntry = Extract<ProgressKind,{type:'agent_activity'}>`, so `Vec<ProgressKind>` is the exact matching schema — no new type). Owner scope (`WHERE user_id`) unchanged.
- **ITEM-4** — verdict: PASS — `AgentActivityTimeline` (workflow/components/run/AgentActivityTimeline.tsx) takes `{stepId, entries: AgentActivityEntry[]}`; `BackgroundRunCard.toggleResult` lazy idiom exists. Render with `entries={detail.activity.filter(e => e.type==='agent_activity')}`.
- **ITEM-5** — verdict: CONCERN — the isolate block is `fanout.rs:273-279`; `NoopEventSink` swap at :275. `AgentCore` is a struct literal (core.rs:250) with construction sites at core.rs:903/1100, tasklist.rs:570, core_tools.rs:643, fanout.rs:473, test_fakes.rs:498, tests/real_llm_loop.rs:163/243, agent_dispatch.rs:1038, dispatcher.rs:221 — ALL need the new `child_sink_factory: None`. DEC narrows the port to `ChildSink{ for_child(child_run_id, label) -> Arc<dyn EventSink>; settle_child(child_run_id, ok) }` (no `ChildParentRef` — the host bakes parent identity into the concrete factory; keeps the crate maximally domain-free). Also add a `settle_child` call at the join barrier so a child-RUN error / panic (which emits no `Stopped`) still marks the child row terminal.
- **ITEM-6** — verdict: CONCERN — floor confirmed: highest server prefix `202608210100`; `subagent` job_kind already valid (202607190700 CHECK). `workflow_runs.conversation_id` FK is `ON DELETE SET NULL` (202607144220:11) — so it does NOT cascade child cleanup. DEC narrows the migration to ONE nullable column `parent_message_id uuid REFERENCES messages(id) ON DELETE CASCADE` + a partial index: `messages` cascade (conversation-delete → messages cascade → child; message regen → child) satisfies "cascade with parent" without a dead `parent_run_id`/`parent_conversation_id` (only chat fan-out is wired — see DEC). Prefix chosen `202608250100`.
- **ITEM-7** — verdict: CONCERN — the factory is injected ONLY where `isolate_children=true`, i.e. the CHAT host (dispatcher.rs:221); the workflow/background hosts (`isolate_children=false`, agent_dispatch.rs:1038) never reach the fanout factory swap, so they stay `child_sink_factory: None` (children unchanged). `insert_subagent_child_run` inserts a `workflow_runs` row with explicit `id=child_run_id` (mirrors `insert_background_run` repository.rs:477 but id-explicit + `parent_message_id`). Factory degrades to a harmless persisting-sink-on-a-missing-row (no-op appends) if the insert fails.
- **ITEM-8** — verdict: CONCERN — the `WorkflowsRead` permission (`workflows`-`read`) is granted to the **Users** system group (202607146095) so `RequirePermissions<(WorkflowsRead,)>` is held by every chat user — reuse it (NO new permission; it already exists, so nothing is introduced). The DETAIL endpoint reuses `get_background_run_detail` (a `subagent` child is background-kind `job_kind<>'workflow'`, so it already 404s cross-user + returns `activity` after ITEM-3). Only the LIST-by-`parent_message_id` is new. Endpoints live in the workflow module (child rows ARE workflow_runs). Mirror `background_mcp/runs.rs` handler+docs shape.
- **ITEM-9** — verdict: PASS — `SubAgentActivityCard` (chat/components/agent-activity) is presentational over `{children:[{id,label,status}]}`; `child.id` IS the child_run_id (fanout `child_id.to_string()`), so the drill-in fetches by it directly. `SubAgentActivity.store.ts` is SSE-only today (its doc even names the durable-view gap this feature fills); add `loadChildTranscript`/`childDetailsById`.
- **ITEM-10** — verdict: PASS — `emit_workflow_run(action, run_id, owner, origin)` exists (events.rs; used by background terminal). `settle_child` emits it owner-scoped, `origin=None`. Card self-gates refetch on `hasPermissionNow(WorkflowsRead)`.
- **ITEM-11** — verdict: PASS — `deps_boundary.rs` enforces the SDK direction; a no-factory unit test (fan-out with `child_sink_factory=None` ⇒ Noop, summary-only) guards INV-3/INV-5. Mirrors the existing `isolate_children_runs_each_child_on_a_fresh_transcript` fanout test.

### Breakage risk
`AgentCore` struct-literal + the 10 construction sites (incl. `tests/real_llm_loop.rs`) — mitigated by `cargo check -p agent-core --tests`. `get_background_run_detail` query rewrite must keep the same owner-scope + 404 semantics (existing background tests guard it). `WorkflowEventSink` refactor must stay byte-identical (TEST-7). No existing caller of the touched fns changes signature except `get_background_run_detail`'s return shape (adds a field; additive).

### Pattern conformance
Persist/read mirrors `workflow/agent_dispatch.rs` + `repository.rs`. agent-core port mirrors the existing `EventSink`/`SteerNotePort` injected-optional-port pattern (ports.rs). REST mirrors `background_mcp/runs.rs` (owner-scoped, foreign→404, `get_with`/`docs`). Migration mirrors an FK-cascade `ALTER TABLE workflow_runs ADD COLUMN … REFERENCES … ON DELETE CASCADE`. Frontend reuses `AgentActivityTimeline` + `BackgroundRunCard.toggleResult`.

### Migration collisions
New prefix `202608250100` > floor `202608210100`; unique (grepped). Server 2026… sequence (not the desktop 1e13 block). FK target `messages` (chat 202607140110) + `workflow_runs` predate it. `cargo clean` after adding so build.rs re-runs.

### OpenAPI regen
YES — `BackgroundRunDetail.activity: Vec<ProgressKind>` + new `SubAgentRunSummary` DTO + 2 subagent-runs routes. `just openapi-regen` (BOTH ui + desktop) at fan-in; golden parity test guards it.
