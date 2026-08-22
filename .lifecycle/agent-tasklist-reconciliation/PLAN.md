# PLAN — agent task-list terminal-state reconciliation

## Design source

This is a bugfix mined from the live UI-exploration audit rig, not a greenfield
feature; the "design" it realizes is the system's own data-integrity contract for
the agent task list plus the defect report.

- Realizes the durable-task-list contract stated in
  `src-app/agent-core/src/tasklist.rs` (DEC-52 header) and
  `src-app/server/src/modules/agent/task_list.rs` (module header): the
  `agent_task_list` store is the **SOURCE OF TRUTH** for an agent run's checklist.
- Realizes the deferred concern named verbatim in the table's own migration
  `src-app/server/src/modules/agent/migrations/202607190810_agent_task_list.sql`
  lines 9-13: `run_id` is polymorphic and "Run-level cascade cleanup on
  conversation/run delete is therefore a deferred retention concern (tracked)".
- Defect report (rig): 41 of 88 `agent_task_list` rows sit `in_progress`/`pending`
  while their `workflow_runs` row is `completed` (35) or `cancelled` (3) (+3
  `pending` under completed runs). User-visible: a finished agent run whose
  checklist claims work is still in progress, forever.

## Invariants

- **INV-1**: The `agent_task_list` store is the SOURCE OF TRUTH for the run's
  checklist — so when a run reaches a terminal state (completed/failed/cancelled/
  timeout), NONE of its task rows may remain in a non-terminal status
  (`pending`/`in_progress`). (Lifted from the store's source-of-truth contract +
  the defect definition.)
- **INV-2**: Reconciliation must be HONEST — a `completed` task row is never
  rewritten (real completion is preserved), and a genuinely-unfinished task is
  driven to a terminal state that does NOT claim the work was done (i.e. NOT
  flipped to `completed`).
- **INV-3**: Reconciliation covers EVERY terminal path, not just clean completion —
  the user-cancel path and the crash/restart-recovery path (where the leak
  actually came from) are reconciled too.
- **INV-4**: A task row must not outlive its run as an orphan — deleting the
  owning `workflow_runs` row removes its task rows (the "cascade cleanup on run
  delete" the table migration deferred).

## Items

- **ITEM-1**: A reconciliation primitive in `agent::task_list` —
  `reconcile_run_terminal(pool, run_id) -> Result<u64>` — that flips every
  `pending`/`in_progress` row of a run to the terminal `abandoned` value and
  leaves `completed` rows untouched. Keyed by `run_id` (the universal key: equals
  `workflow_runs.id` for workflow/background runs).
- **ITEM-2**: The honest terminal vocabulary `abandoned` — widen the
  `agent_task_list_status_check` CHECK to include it (ITEM-6 migration), add a
  `TaskStatus::Abandoned` variant in `agent-core` so a store read round-trips
  honestly (otherwise `status_from_str` degrades an unknown value back to
  `Pending` and the fix is invisible on read), render it (`[-]`), and map it in
  the server `status_to_str`/`status_from_str`. NOT added to the model-settable
  `status_schema()` — abandonment is a system reconciliation act, not a
  model-issued status.
- **ITEM-3**: Hook the workflow/background LIVE terminal chokepoint
  `repository::mark_status` — when it transitions a run to a terminal status
  (`is_terminal()`) and actually flips the row, reconcile that run's task list.
  This single site covers `run_workflow`, `run_test`, and `spawn_background_run`
  completed/failed/cancelled/timeout arms.
- **ITEM-4**: Hook the user-cancel path `repository::cancel_cas` — when a cancel
  actually lands (returns `Some`), reconcile that run's task list. (The rig's 3
  `cancelled` cases route here, bypassing `mark_status`.)
- **ITEM-5**: A startup self-healing bulk sweep in `startup_sweep::sweep_at_boot`
  (right after `fail_orphaned_runs`) that abandons the open task rows of ANY
  terminal `workflow_runs` row. This covers the crash/restart-recovery path
  (orphaned runs swept to `failed`) AND retroactively remediates the pre-existing
  leaked rows (the rig's 41) on next boot. Implemented as a bulk
  `repository::reconcile_orphaned_task_lists(pool)`.
- **ITEM-6**: Migration (above the current server max `202607200600`): (a) widen
  `agent_task_list_status_check` to include `abandoned`; (b) add a SEPARATE
  nullable `workflow_run_id uuid REFERENCES workflow_runs(id) ON DELETE CASCADE`
  column (run_id itself CANNOT carry an FK — it is polymorphic across chat message
  ids, workflow_runs ids, and fan-out child ids, verified in code); (c) index the
  FK column; (d) backfill `workflow_run_id = run_id` for existing rows whose
  run_id is a real `workflow_runs` id. Plus populate `workflow_run_id` on new rows
  in `PgTaskListStore::create` via an existence-guarded subquery
  `(SELECT id FROM workflow_runs WHERE id = $1)` so chat/fan-out rows stay NULL
  and never violate the FK.
- **ITEM-7**: [DESCOPED] Chat agent-host live reconciliation hook
  (`chat/agent_host/dispatcher.rs`). The chat agent-core path is behind the
  non-default `ZIEE_CHAT_AGENT_CORE=1` flag (experimental, "for behavioral
  verification"), unmeasured by the rig, and its task rows (keyed by an
  assistant message id, with no `workflow_runs` row) have no crash-recovery model.
  The reconcile primitive (ITEM-1) is keyed by `run_id` so it already supports
  that key shape if the path graduates. See DECISIONS.md DEC-6.

## Files to touch

- `src-app/server/src/modules/agent/migrations/202608210100_agent_task_list_reconcile.sql` (NEW — ITEM-6)
- `src-app/server/src/modules/agent/task_list.rs` (ITEM-1 primitive; ITEM-2 status_to/from_str; ITEM-6 create() workflow_run_id)
- `src-app/agent-core/src/types.rs` (ITEM-2 `TaskStatus::Abandoned`)
- `src-app/agent-core/src/tasklist.rs` (ITEM-2 `render_list_lines` arm)
- `src-app/server/src/modules/workflow/repository.rs` (ITEM-3 mark_status hook; ITEM-4 cancel_cas hook; ITEM-5 `reconcile_orphaned_task_lists`)
- `src-app/server/src/modules/workflow/startup_sweep.rs` (ITEM-5 call in sweep_at_boot)
- `src-app/server/src/modules/chat/core/types/streaming.rs` (ITEM-2 — `TaskListItemStatus::Abandoned`, keeps `From<TaskStatus>` total + faithful; see DRIFT-1.1)
- `src-app/server/src/lib.rs` (`test_internals` exports of the reconcile fns + terminal writers; see DRIFT-1.2)
- `src-app/server/tests/agent/task_list_test.rs` (tests — extend existing)
- Regenerated (deterministic): `src-app/server/openapi/openapi.json`, `src-app/ui/openapi/openapi.json`, `src-app/{ui,desktop/ui}/src/api-client/types.ts` (ITEM-2 wire-enum value; see DRIFT-1.1)

## Patterns to follow

- **Reconcile primitive + create() change** — mirror the existing
  `src-app/server/src/modules/agent/task_list.rs` style: runtime `sqlx::query` /
  `query_as` (NOT compile-time `query!` macros, per that file's header), map errs
  with `AppError::database_error`.
- **Terminal-writer hooks** — mirror the existing chokepoint discipline in
  `src-app/server/src/modules/workflow/repository.rs` (`mark_status`, `cancel_cas`,
  `fail_orphaned_runs` all live there and already use `sqlx::query!`).
- **The separate `workflow_run_id` FK column** — mirror the existing precedent of
  `mcp_tool_calls.workflow_run_id` / `scheduled_task_runs.workflow_run_id`
  (nullable FK to `workflow_runs`) and the run-scoped-CASCADE precedent
  `background_run_notes.run_id` / `file_workflow_runs.workflow_run_id`
  (`ON DELETE CASCADE`). Task-list rows are ephemeral run-scoped working state →
  CASCADE (like `background_run_notes`), not SET NULL (which is for audit/history
  rows that outlive the run).
- **`TaskStatus` enum extension** — mirror the existing three-variant enum in
  `src-app/agent-core/src/types.rs`; the compiler enforces exhaustive-match
  updates everywhere it is consumed.
- **Tests** — extend `src-app/server/tests/agent/task_list_test.rs`, reusing its
  `pool(&server)` + `insert_item(...)` helpers and the `TestServer` harness.

---

# Plan audit (phase 2) — verified against the codebase

## Breakage risk

- Adding `TaskStatus::Abandoned` forces exhaustive `match` updates. Verified
  consumers: `render_list_lines` (tasklist.rs:224), server `status_to_str`/
  `status_from_str` (task_list.rs:52-68). `test_fakes.rs` uses
  `unwrap_or(Pending)` (no match). The compiler catches any missed arm — low risk.
- `mark_status` gains a post-UPDATE reconcile call gated on `is_terminal()` +
  actual row flip; non-terminal transitions (via `mark_running`, resumable/waiting
  writers) are untouched. `cancel_cas` reconcile is gated on a `Some` return.
- `create()` INSERT gains one column via an existence-guarded subquery — cannot
  raise an FK violation for chat/fan-out run_ids (subquery yields NULL). Existing
  behavior for the returned `TaskItem` is unchanged (workflow_run_id not in the
  RETURNING/`TaskItem` shape).

## Pattern conformance

- Reconcile primitive + `create()` change use runtime `sqlx::query`/`query_as`
  per the `task_list.rs` header convention. PASS.
- The separate nullable `workflow_run_id` FK column mirrors
  `mcp_tool_calls`/`scheduled_task_runs` (FK-to-workflow_runs column) and the
  CASCADE choice mirrors `background_run_notes`/`file_workflow_runs`. PASS.
- Terminal-writer hooks live in `workflow/repository.rs` beside the writers they
  guard (existing coupling workflow→agent already present in `agent_dispatch.rs`).
  PASS.

## Migration collisions

- New prefix `202608210100` > current server max `202607200600`; unique across
  `src-app` (checked). Cross-module FK to `workflow_runs` (created
  `202607140230`) is safe under the single global timestamp ordering, matching
  the existing cross-module FKs (`mcp` → `workflow_runs`). No collision.

## OpenAPI regen

- REQUIRED (amended — see DRIFT-1.1). `agent_task_list` itself has no REST
  surface, but adding `TaskStatus::Abandoned` forces `TaskListItemStatus`
  (the chat-streaming wire DTO, `#[derive(schemars::JsonSchema)]`) to gain the
  same value to keep its `From<TaskStatus>` total + honest. That is a schema
  delta → `just openapi-regen` (both binaries) + the `emit_ts` golden parity
  test must stay green. FE consumers: none import the generated
  `TaskListItemStatus`, so no hand-written UI change and tsc stays green (still
  not classified as UI work — the generated files are excluded from the UI-diff
  gate).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new free fn in `task_list.rs`; mirrors file's runtime-query style; no existing caller affected.
- **ITEM-2** — verdict: PASS — enum extension is compiler-checked; `status_schema()` deliberately left at the 3 model-settable values (verified it is a plain literal array, not derived from the enum, so no drift). NOTE (DRIFT-1.1): the wire DTO `TaskListItemStatus` also gains the value → OpenAPI regen required; FE union is hand-written + separate, so no FE break.
- **ITEM-3** — verdict: PASS — `mark_status` is the single guarded chokepoint for the runner terminal arms (verified: run_workflow 507/531/555, run_test 1150/1163/1172, spawn_background_run 1476/1480/1489 all route through it).
- **ITEM-4** — verdict: PASS — `cancel_cas` (repository.rs:1091) is the user-cancel writer that bypasses `mark_status`; returns `Option<String>` to gate the reconcile.
- **ITEM-5** — verdict: PASS — `sweep_at_boot` (startup_sweep.rs:33) already runs at boot after `fail_orphaned_runs`; a bulk reconcile there covers crash-recovery + retroactive remediation.
- **ITEM-6** — verdict: CONCERN — cross-module FK + CHECK-widen migration; requires `cargo clean` so build.rs re-migrates the per-worktree build DB. Handled in phase 8. No `query!` macro depends on the new column (reconcile/create use runtime queries), so compilation is not blocked pre-clean.
- **ITEM-7** — verdict: PASS — [DESCOPED] with approved disposition in DECISIONS.md (DEC-6); the primitive already supports the chat key shape.
