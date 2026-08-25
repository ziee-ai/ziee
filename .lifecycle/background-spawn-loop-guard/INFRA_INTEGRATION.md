# INFRA_INTEGRATION — background-spawn-loop-guard

## User-experience walk

A user (or its chat model) asks for background work. The model calls
`spawn_background`. Normal path: approval → guard passes (fresh spec, under cap) →
a run starts (`status: pending`), the turn ends, the user is re-engaged on
completion. Loop path (the defect): the model, re-triggered by a
`[Background task complete]` re-injection, calls `spawn_background` with the SAME
spec. Now: the guard returns a NON-error result `status: already_running` carrying
the existing run_id — the model reads "already running, do not spawn again" and
stops; no second run is created. If the model instead floods DISTINCT tasks past
the cap, it gets a clear `BACKGROUND_SPAWN_CAP_EXCEEDED` error and no run. In both
cases the user's conversation is not buried under duplicate/parallel runaway runs.

## Infrastructure-integration walk

- **MCP tool-call + approval flow** — `spawn_background` is already approval-gated
  (`background_call_needs_approval` returns true). The guard runs INSIDE the tool
  handler (after approval), so a duplicate spawn is still approval-prompted then
  refused; the guard is the run-creation backstop, not an approval change. Deemed
  acceptable: the CAUSE fix is "no duplicate run", not "no duplicate prompt";
  changing the approval layer is out of scope and higher-risk. Noted, not a gap.
- **workflow_runs backbone / runner** — the guard reuses the existing
  `insert_background_run` INSERT (factored to a tx-capable helper) and the existing
  `spawn_background_run` register/mark_running/heartbeat/terminal machinery
  unchanged; Duplicate/OverCap short-circuit BEFORE any handle is registered or
  task spawned, so no orphan handle and (for OverCap) no row.
- **Sync (notify-and-refetch)** — Duplicate/OverCap create no new run, so no
  `SyncEntity::WorkflowRun` emit is needed (nothing changed). The normal Spawned
  path emits exactly as before.
- **agent_admin_settings** — the cap reads `fan_out_max_threads` via the existing
  `Repos.agent.get_admin_settings()` (the same read `drive_subagent_turn` already
  does). One extra DB read per spawn; spawn is not a hot path (it already fetches
  the conversation + resolves a model), so acceptable. An unreadable row → default
  6, clamped ≥1, so the guard can never wedge all spawns.
- **Cancel-on-conversation-delete / startup sweep** — untouched; the guard only
  affects run CREATION.
- **Scheduler / detached callers** — `spawn_background_run` with `guard: None`
  (conversation-less) is byte-identical to before (Spawned), so scheduled/detached
  background work is unaffected.

## Entity-lifecycle walk

- **A background run row** — ADD is now guarded (dedup/cap). A Duplicate returns
  an EXISTING run_id; that row is read live at query time inside the locked txn, so
  the returned id is valid when returned. REMOVE/terminal transitions are unchanged
  (the guard's dedup deliberately excludes `cancelled` and old terminal rows, so a
  deleted/cancelled/old run never blocks a legitimate re-spawn). Access-loss: the
  guard is owner-scoped (`user_id` in every query), so it never reads or dedups
  against another user's runs.
- **Concurrency (local + racing)** — two racing identical spawns for the SAME
  conversation serialize on `pg_advisory_xact_lock(conversation)`: the first
  inserts + commits, the second then sees the committed row and dedups. Proven by
  design (the acceptance tests seed the "first already exists" state directly, and
  the T2 concurrency-resource audit angle re-checks the TOCTOU path).
