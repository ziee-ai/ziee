# TEST_RESULTS — agent task-list terminal-state reconciliation

Backend-only diff (no UI workspace touched — the real `src-app/ui` spec is
unchanged; `abandoned` never reaches the wire), so no `npm run check` / gate:ui /
e2e lines apply. Full logs under `/data/pbya/ziee/tmp/lifecycle-logs/`.

## Enumerated tests (phase 3)

- **TEST-1**: PASS — `reconcile_marks_open_rows_abandoned_on_completion` (integration). Open rows → `abandoned`, completed preserved, via the REAL `mark_status(Completed)`.
- **TEST-2**: PASS — `reconcile_leaves_all_completed_run_untouched` (integration, now discriminating: 2 completed + 1 open; open → abandoned, completed preserved, exactly 1 abandoned).
- **TEST-3**: PASS — `reconcile_covers_cancel_crash_and_retroactive_paths` (integration: cancel_cas + fail_orphaned_runs+bulk-sweep + retroactive under a completed run) AND `sweep_at_boot_reconciles_orphaned_task_rows` (the production boot-wiring).
- **TEST-4**: PASS — `workflow_run_id_fk_populates_and_cascades` (integration: NULL for a chat-shaped run_id, set for a workflow run_id, DELETE cascades the task rows).
- **TEST-5**: PASS — server `status_roundtrips_through_db_vocabulary` (`abandoned` round-trips, no degrade to Pending) + agent-core `abandoned_item_renders_dash_mark` (`[-]`) + `model_cannot_set_abandoned_status` (guard rejects a model-set abandoned on create AND update).
- **TEST-6**: PASS — `reconcile_marks_open_rows_abandoned_on_failure` (integration: the FAILED terminal arm also reconciles).

## Run evidence

- Integration (`agent::task_list`, `--test-threads=1`): `test result: ok. 8 passed; 0 failed`
  — `agent-tasklist-int2.log`.
- agent-core unit (`tasklist::`): `test result: ok. 9 passed; 0 failed`
  — `agent-tasklist-unit-agentcore.log`.
- server-lib unit (`agent::task_list::`): `test result: ok. 2 passed; 0 failed`
  (status round-trip incl. `abandoned`; deps json) — `agent-tasklist-unit-server.log`.
- `cargo check -p ziee --tests`: exit 0, 0 errors — `agent-tasklist-check2.log`.

## Mutation check (proves the tests are real, not tautological)

Neutered the `mark_status` reconcile (`if false && …`) and re-ran TEST-1:
`test result: FAILED. 0 passed; 1 failed` —
`assertion left == right failed: pending → abandoned at run end` (left was
`"pending"`). Restored via `git checkout`. — `agent-tasklist-mutation.log`.
The strengthened TEST-2 is likewise mutation-sensitive by construction (an open
row present forces the hook to fire), and the `sweep_at_boot` wiring test goes RED
if the boot reconcile call is removed.

## Deterministic phase-8 checks

- A2 clean tree; A3 no diff-added `#[ignore]`/`.skip`; A4 no cosmetic assertions.
- Acceptance tests (INV-1..INV-4) all recorded PASS above (TEST-1..TEST-4).
- A11: TEST-1..TEST-6 each appear on an added line of `git diff <base>...HEAD`.
- No new permission (no A9/A10), no new built-in MCP server (no A8), no UI (no A7).
