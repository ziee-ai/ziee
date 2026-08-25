# PLAN — background-spawn-loop-guard

## Problem (diagnosis)

`background_mcp`'s `spawn_background` tool lets the chat model launch detached
background runs (`workflow_runs`, `job_kind='subagent'|'sandbox_exec'`). In a live
conversation the model spawned the SAME task repeatedly — the DB shows 9
`job_kind=subagent` runs, 6–7 of them the IDENTICAL "Write a Python script to
generate the Mandelbrot set…" spec. The transcript shows the loop: each
`[Background task complete] …` completion re-injected into the conversation
re-triggers the model, which re-spawns the same work. There is NO spawn
de-duplication and NO per-conversation spawn cap, so a confused (esp. weaker
local) model spawns unboundedly.

Fix the CAUSE at the spawn boundary: the backend refuses duplicate/over-cap
spawns with a clear message. A refused duplicate is what breaks the completion→
respawn loop (we do NOT try to "fix the model").

## Design source

Realizes `agent-kit/docs/CODING_GUIDELINES.md` §4 (bounded deletes/selects — no
unbounded-growth path), §5 (resource lifecycle — every accumulating thing has a
bound + a clear refusal), and §6 (no silent swallow — return a clear error),
applied to the `background_mcp` `spawn_background` boundary per the diagnosis
above. No new upstream design doc existed for this hardening fix; the binding
intent is the three invariants below, lifted from the CODING_GUIDELINES sections
and the diagnosis.

## Invariants

- **INV-1**: two `spawn_background` calls with the same (conversation, task-spec
  fingerprint) within the active/recent window create only ONE run — spawn is
  idempotent; the duplicate returns a clear "already running/queued" result
  instead of a second run.
- **INV-2**: the number of concurrent+queued background runs per conversation is
  bounded by a cap; an over-cap `spawn_background` returns a clear over-cap error
  and creates NO run (CODING_GUIDELINES §4/§5).
- **INV-3**: a `[Background task complete]` resume injection never itself causes a
  new spawn — completion feedback is not a spawn trigger (no self-perpetuating
  loop).

## Items

- **ITEM-1**: Add a race-safe guarded background-run insert to the workflow
  repository: within ONE transaction holding a per-conversation
  `pg_advisory_xact_lock`, (a) look up an existing same-(conversation, job_kind,
  inputs_json) run in the dedup window, (b) count non-terminal background runs for
  the conversation against the cap, (c) INSERT only if neither guard fires. Return
  a three-way outcome (Inserted / Duplicate(existing_id) / OverCap{active}). Reuse
  the existing `insert_background_run` SQL (factored to a shared tx-capable
  helper) so the row shape cannot drift.
- **ITEM-2**: Thread the guard through `runner::spawn_background_run`: accept an
  optional `BackgroundSpawnGuard { max_active_per_conversation, dedup_window_secs }`
  and return a `BackgroundSpawnResult { Spawned(id) | Duplicate(id) | OverCap{active,cap} }`.
  When a guard is supplied AND the run is conversation-bound, route through the
  guarded insert; Duplicate/OverCap short-circuit BEFORE any task is spawned (no
  handle registered, no run row for OverCap). No guard (or no conversation) →
  unchanged behavior (Spawned), preserving detached/scheduler callers.
- **ITEM-3**: In `background_mcp::tools`, build the guard for every conversation-
  bound spawn (both `subagent` and `sandbox_exec`), reading the cap from the
  existing `agent_admin_settings.fan_out_max_threads`; map the guarded result to
  the model-facing tool response — Spawned → the existing pending handle;
  Duplicate → a clear "already running/queued" result carrying the existing
  `run_id` (INV-1); OverCap → a clear `BACKGROUND_SPAWN_CAP_EXCEEDED` error
  creating no run (INV-2).
- **ITEM-4**: Verify + assert INV-3: the `[Background task complete]` resume
  injection (`resume::build_resume_message`) carries no spawn-inducing directive,
  and a recently-COMPLETED identical spec re-spawn (what the re-engaged model
  attempts) is refused as a duplicate — so completion feedback creates no new run.

## Files to touch

- `src-app/server/src/modules/workflow/repository.rs` — factor the background
  INSERT into a tx-capable helper; add `insert_background_run_guarded` + its outcome enum (ITEM-1).
- `src-app/server/src/modules/workflow/runner.rs` — guard param + result enum on
  `spawn_background_run` (ITEM-2).
- `src-app/server/src/modules/background_mcp/tools.rs` — build guard, map result
  to tool response, dedup window const (ITEM-3).
- `src-app/server/src/modules/background_mcp/resume.rs` — INV-3 assertion (unit
  test that the resume message carries no spawn directive) (ITEM-4).
- `src-app/server/tests/background_mcp/spawn_guard.rs` (new) — acceptance
  integration tests for INV-1/INV-2/INV-3 driving the real `/api/background/mcp`
  spawn boundary.
- `src-app/server/tests/background_mcp/mod.rs` — register the new test module.
- `src-app/server/src/modules/workflow/repository.rs` test `spawn_background_run_drives_to_terminal`
  — update to the new signature (pass `None` guard, match `Spawned`).

## Patterns to follow

- **Race-safe check-then-insert** — mirror CODING_GUIDELINES §4 "guard+write in
  one txn"; use `pg_advisory_xact_lock` keyed on the conversation (Postgres, one
  cluster shared across worktrees — advisory locks are process/txn-scoped, safe).
- **Guarded terminal write precedent** — `workflow/repository.rs::mark_status` /
  `cancel_cas` (status-guarded CAS in one statement) for the "first-writer-wins /
  atomic check" idiom.
- **Reuse an existing operational tunable** — the cap reuses
  `agent_admin_settings.fan_out_max_threads` (read via `Repos.agent.get_admin_settings()`,
  the same read `drive_subagent_turn` already does), so NO migration/permission/
  admin-card is added (keeps LIGHT tier).
- **Model-facing refusal shape** — mirror the existing `spawn_background`
  refusals in `tools.rs` (`AppError::bad_request(CODE, actionable message)`); a
  Duplicate is a normal result (not an error) so the model reads it as "already
  running, stop".
- **Acceptance integration tests** — mirror `tests/background_mcp/spawn_contract.rs`
  (real JSON-RPC over `/api/background/mcp`, `x-conversation-id`, direct
  `workflow_runs` count/seed via an ad-hoc pool that is `close()`d).

## UI-surface checklist

N/A — this is a BACKEND-only change (no `src-app/ui/**` / `src-app/desktop/ui/**`
diff, no new REST endpoint, no OpenAPI/type change). The frontend gates
(phase-3 e2e requirement, phase-8 `npm run check`/`gate:ui`) therefore do not
apply. Confirmed against the enumerated Files to touch.

## Plan audit (phase 2 — verified against the codebase)

### Breakage risk
Changing `runner::spawn_background_run`'s return type (`Uuid` → `BackgroundSpawnResult`)
and adding a `guard` param breaks exactly its 3 call sites: two in
`background_mcp/tools.rs` (mine, ITEM-3 rewrites them) and one in-module test
`spawn_background_run_drives_to_terminal` (updated to `None`/`Spawned`). Verified
via `grep -rn spawn_background_run` — no other production or test caller. The new
guarded insert is additive. No external/API caller is affected.

### Pattern conformance
`insert_background_run_guarded` mirrors the existing guarded-write idiom
(`mark_status`/`cancel_cas`, single-statement/one-txn atomic checks); the cap reuse
mirrors `drive_subagent_turn`'s existing `Repos.agent.get_admin_settings()` read;
the acceptance tests mirror `tests/background_mcp/spawn_contract.rs` (real JSON-RPC
+ ad-hoc pool `run_count`). Conforms.

### Migration collisions
None — no migration added (see BASE.md). Highest server migration `202608250100`
is untouched.

### OpenAPI regen
Not required — no REST/schema type change (see BASE.md). The `spawn_background`
MCP result is free-form JSON, not in `openapi.json`.

- **ITEM-1** — verdict: PASS — additive repo fn; reuses the `insert_background_run` INSERT (factored to a tx helper); jsonb `=` gives canonical spec equality with no schema change; `pg_advisory_xact_lock` is the §4 atomic-guard idiom.
- **ITEM-2** — verdict: PASS — signature change reaches only 3 known call sites; guard optional so detached/scheduler callers are byte-identical (Spawned).
- **ITEM-3** — verdict: PASS — reuses the existing agent-settings read for the cap (no new migration/permission → LIGHT); refusal shapes mirror the existing `spawn_background` refusals.
- **ITEM-4** — verdict: PASS — `resume::build_resume_message` verified to end with "Use this result to continue the conversation" (no spawn directive); INV-3 additionally enforced structurally by the recent-completed dedup window (ITEM-1) + the existing spawn-approval gate on the resumed turn (resume.rs:205-209).
