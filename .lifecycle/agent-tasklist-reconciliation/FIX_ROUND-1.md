# FIX_ROUND-1 — resolving the phase-6 blind-audit ledger

Three blind angles ran over `git diff origin/main...HEAD`: **correctness**, **db**
(migration conditional), **design-conformance** (required). Findings triaged in
`LEDGER.jsonl`. Fixes this round:

- **Partial-success on the terminal writers** (correctness + design-conformance,
  corroborated=2): `mark_status` and `cancel_cas` chained the reconcile with `?`
  after the terminal write had already committed, so a task-list DB hiccup wrongly
  failed the run-terminal transition. Fixed: the reconcile is now best-effort
  (log `warn!`, don't propagate) — the run's terminal state is the primary write;
  the boot sweep is the backstop. Mirrors the fire-and-forget MCP tool-call
  recorder posture.
- **Model could set the system-only `abandoned`** (correctness, contradicts DEC-2):
  added `reject_model_set_abandoned` guard in `handle_task_create`/`handle_task_update`
  so a model-supplied `status:"abandoned"` is refused with a clear error. `abandoned`
  stays system-assigned only.
- **`sweep_at_boot` production wiring untested** (design-conformance): added
  `sweep_at_boot_reconciles_orphaned_task_rows`, which drives the REAL boot entry
  point (exposed via `test_internals`) — deleting the reconcile call in
  `sweep_at_boot` now turns a test RED.
- **INV-2 acceptance test non-discriminating** (design-conformance): TEST-2 now
  includes an open row alongside the completed ones, so the hook MUST fire — it
  fails if the hook is deleted (open row not abandoned) OR if reconcile wrongly
  touches completed rows.
- **Full index over a mostly-NULL FK column** (db): `idx_agent_task_list_workflow_run`
  is now a partial index (`WHERE workflow_run_id IS NOT NULL`), matching the
  `mcp_tool_calls.workflow_run_id` precedent.

Rejected / wontfix (recorded in the ledger, not silently dropped):
- **TOCTOU** (correctness, low): a task row created by a still-live loop after
  reconcile — but at a terminal transition the loop has already returned; the boot
  sweep is the backstop. Not worth a per-run lock.
- **Chat agent-host gap** (design-conformance + db, corroborated=2): DESCOPED
  (DEC-6, human-approved) — flag-gated non-production path, out of the task's named
  scope; with the flag off zero chat rows exist. DEC-6 strengthened with the
  no-backstop caveat + the one-line graduation fix.

**New confirmed findings:** 0
