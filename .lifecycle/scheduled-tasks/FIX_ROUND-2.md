# FIX_ROUND-2 — scheduled-tasks

A second **blind** multi-angle round was run against the CURRENT full diff
(`git diff origin/main...HEAD`, 37 files) after Phase 8 added the frontend tests,
the typed-helper extraction, and the two enumerated UI additions (Completed badge
+ skipped-tools note). Diff-only context, no author reasoning shared. All 12
angles applied; coverage regenerated so every hunk is reviewed by ≥3 angles.

## Result of the round

**No new med/high confirmed defects.** Every security/correctness/authz probe held:
- unattended double-gate (non-allow-listed servers neither attach via `mcp_config`
  nor execute — deny-not-pause), fire-time model re-check, fail-closed
  `get_user_defaults` on the workflow standalone tool-step path;
- `preemptive_pause_reason` correctly distinguishes a failed first fire from a
  deleted conversation via `last_status`;
- `count_active_for_user` excludes the row being re-enabled (no off-by-one);
- the `forwardRef` pickers' declared prop surface matches exactly what kit
  `FormField`/`Controller` injects (value / onChange(value) / aria-*).

## One LOW finding — fixed

- **(api-contract, LOW)** `tests/e2e/14-scheduler/paused-and-runs.spec.ts` mocked a
  run's `skipped_tools` as `{ server_id, tool_name }`, but the backend schema is
  `SkippedTool { tool_name, reason }` (`scheduler/models.rs`). Harmless at runtime
  today (the FE `skippedToolsNote` only reads array length), but an infidelic
  fixture that wouldn't catch a future FE change reading `.reason`/`.tool_name`.
  **Fixed:** the mock now uses `{ tool_name, reason }` matching the real schema;
  TEST-32 re-run green (the note still renders "2 tools skipped").

The fix touches ONLY a test fixture's mock data shape (no runtime/source code),
so it cannot introduce a new runtime defect; the round therefore converges here.

**New confirmed findings:** 0
