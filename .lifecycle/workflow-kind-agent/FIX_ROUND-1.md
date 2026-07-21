# FIX_ROUND-1 — workflow-kind-agent

After fixing the 22 Phase-6 findings, a FULL blind re-audit round (2 fresh diff-only agents on the fix
commit) surfaced **5 new confirmed findings — all regressions/gaps in the fixes themselves** (0 high,
4 medium, 1 low). Each is now fixed; the rest of the fix surface was explicitly cleared by the auditors
(save routing, 409 error-shape, cloneElement a11y, buffer resync, timeline merge order).

## New findings this round + resolution
- **R1-1 / R1-2 (MED, backend `dev.rs`) — non-atomic update regression.** FIX-1 had replaced the
  destructive extract with an IN-PLACE `workflow.yaml` overwrite, so a partial write OR a later
  `pack_workspace_dir_measured` failure corrupted the live bundle while the DB kept old metadata (the
  runner re-parses on-disk yaml → unrunnable, no rollback). **Fixed:** rebuilt the update as a
  staging-dir + atomic-swap — validate against the intact live bundle → copy the bundle into a unique
  sibling staging dir → overwrite only `workflow.yaml` there → measure + compile on staging → DB commit
  → atomically rename staging into place (with restore-on-swap-failure). The LIVE bundle is untouched on
  EVERY pre-commit failure path; staging is cleaned up on all exits.
- **R1-3 (LOW, backend) — create-409 TOCTOU.** Two concurrent same-name creates both passed the
  pre-check → the loser's INSERT hit the unique index → generic 500. **Fixed:** `repository::insert`
  now maps the Postgres unique-violation (23505) to the same 409 `WORKFLOW_NAME_EXISTS` (mirrors the
  citations/hub pattern), so the concurrent loser also gets the typed 409.
- **R1-4 (MED, FE `ToolStepForm`) — arg-type corruption regression.** FIX-D's blanket `JSON.parse`
  coerced string args that look like JSON (`"1234"`→number) and re-parsed ALL rows on any edit
  (editing row A corrupted row B). **Fixed:** each row now tracks `text`/`value`/`baseText`; an
  untouched row (`text === baseText`) re-emits its exact loaded typed value with NO re-parse, and
  `toText` is round-trip-stable (quotes a string that would otherwise parse to a non-string). Loaded
  `{zip:"1234", limit:10, topic:"{{ inputs.x }}"}` now survives an unrelated edit with types intact.
- **R1-5 (MED, FE `stepForms.ts`) — vacuous compile-time guard.** The `StepDef` drift assertion was a
  tautology (`(StepBase & StepDef) extends StepDef` is always true), guarding nothing while claiming
  drift protection. **Fixed:** removed the fake assertion + false comments; documented honestly that a
  compile-time guard is impossible while the generated `StepDef` is flatten-lossy, the wire is correct
  (serde-flatten emits every field), and drift-safety is provided by the backend def→bundle round-trip
  integration test (+ the emit_ts non-lossy generator as the tracked follow-up).

## Build after round-1 fixes
cargo check `-p ziee` clean; tsc 0; colors + logical-direction lint pass.

**New confirmed findings:** 5
