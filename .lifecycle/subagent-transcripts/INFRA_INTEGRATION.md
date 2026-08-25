# INFRA_INTEGRATION — the three mandatory per-item walks (phase 5)

Authored live during implementation. UX + infra-integration + entity-lifecycle
walks for each item, focusing on the subsystems each touches.

## UX walk (what a real user does)

- **Background transcript (ITEM-2/3/4)**: after my agent ran something in the
  background, I open the background run's card, expand its result, and read
  exactly what it thought and did — even after a reload. Surfaces: the background
  card (result + a lazily-expanded `AgentActivityTimeline`); empty (no-activity)
  → no timeline block; mobile 390px → the existing responsive timeline.
- **Fan-out child drill-in (ITEM-9)**: after my agent delegated to sub-agents, I
  see the N children in `SubAgentActivityCard` with live status, and I click one
  to read its FULL transcript inline — and it survives a reload (the durability
  fix the store's own doc-comment named as the gap). Surfaces: the chat sub-agent
  card (per-child expand → timeline), a pruned child (404 → status-only, no
  crash), empty/loading/error on the drill-in fetch.

## Infra-integration walk (subsystems each item touches)

- **agent-core seam (ITEM-5)** — touches the fan-out `isolate_children` path ONLY.
  Verified: (a) the factory is consulted exclusively inside that block, so
  non-isolate hosts (workflow/background) are byte-identical (guarded by TEST-5);
  (b) `settle_child` is called at the join barrier for EVERY spawned child incl. a
  child-RUN error / panic (which emits no `Stopped`), so a failed child row is
  still marked terminal — this is why `settle_child` exists rather than the sink
  observing its own `Stopped`; (c) the crate stays DB-free — only `Uuid`/`&str`
  cross the seam (TEST-11 deps_boundary).
- **Activity persistence (ITEM-1/2)** — touches `append_agent_activity` (the
  seq-ordered bounded ring, cap `AGENT_ACTIVITY_MAX_ENTRIES=500`) + the SSE
  `ProgressEmitter`. Verified: the shared `map_agent_event` reproduces
  `WorkflowEventSink`'s exact entries (TEST-7 regression), and `PersistingActivitySink`
  AWAITS each append (not fire-and-forget) so activity is durable BEFORE
  `settle_child` marks the run terminal — no lost-tail race.
- **MCP / approval flow** — NOT touched: fan-out children run with
  `allow_delegate=false` (depth cap) unchanged; the persistence sink is orthogonal
  to tool approval. Background runs keep their `UnattendedDenyGate`.
- **Sync (ITEM-10)** — the child `for_child`/`settle_child` emit owner-scoped
  `WorkflowRun` Create/Update via the SAME `emit_workflow_run` background terminal
  uses. Owner audience routes by `user_id`, so a foreign user is never notified;
  the refetch endpoint is owner-scoped (TEST-10). Background-run activity rides the
  existing `WorkflowRun` sync on the run row (no new emit).
- **Permissions** — NO new permission. The subagent-runs endpoints reuse
  `WorkflowsRead` (held by the Users group); owner-scope by `user_id` is the
  boundary (A9/A10 do not apply — confirmed DEC-14).
- **Retention / DB** — child rows are ordinary `workflow_runs` rows
  (`job_kind='subagent'`), pruned by the EXISTING workflow-run retention (no new
  setting, DEC-9). The migration is additive (two nullable columns + a partial
  index); it back-fills nothing.

## Entity-lifecycle walk (ADD / REMOVE / DELETE / access-loss, BOTH paths)

The surface holds: a background run, a fan-out child run, and the child's
transcript. For each:

- **Child run — CREATE**: `for_child` inserts the row (local path); the FE learns
  of it via the SSE fan-out snapshot (live) + a `WorkflowRun` Create sync
  (cross-device refetch).
- **Child run — access-loss / DELETE (the mandatory prove-by-running case)**: a
  child whose parent conversation is deleted CASCADE-deletes (TEST-8, RUN and
  asserted: count 1 → 0 on `DELETE FROM conversations`). After that delete, the
  drill-in endpoint 404s — the FE MUST render status-only, no crash. This is the
  explicit entity-lifecycle requirement handed to the FE agent (ITEM-9) and its
  TEST-13 asserts the pruned-child (404) status-only render. The BACKEND half is
  proven: `get_background_run_detail` returns `None` → 404 (TEST-4 owner-scope,
  and a pruned id is simply absent → 404).
- **Child run — MUTATE (settle)**: `settle_child` flips status running→terminal
  from the local path; the owner refetches on the `WorkflowRun` Update sync
  (TEST-10). The FE keeps live status from the SSE frame; the persisted status is
  the after-reload source.
- **Background run — access-loss**: a foreign user → 404 (TEST-1's owner-scope
  case), matching the existing `get_background_run_detail` boundary.
- **Insert-failure degradation**: if `insert_subagent_child_run` fails, `for_child`
  still returns a `PersistingActivitySink` whose appends no-op on the missing row
  (UPDATE … WHERE id affects 0 rows) — the child still RUNS, it just has no
  persisted transcript. No panic, no failed fan-out.
