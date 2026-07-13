# DRIFT-1 — implementation vs plan

- **DRIFT-1.1** — verdict: impl-wins — PLAN/TESTS said "add a `google` entry to
  the shared `manifest()` helper" in the integration tests. Implemented as an
  isolated `google_manifest(mode)` helper instead, leaving the shared MCP
  `manifest()` untouched. Rationale: keeps the google tests self-contained and
  avoids perturbing the existing MCP idempotency tests' world (B3 — don't reshape
  shared fixtures for one feature's needs). `env_for` still gains the two
  `GOOGLE_*` vars (needed so `reboot()` carries them), which is harmless to the
  MCP-only manifests (they have no `auth_providers` block). Plan intent (a google
  integration test that boots the real path) is fully satisfied. No PLAN item
  changes; TESTS.md TEST-5/6/7 still map to `tests/desired_state/mod.rs`.

- **DRIFT-1.2** — verdict: resolved — Discovered a PRE-EXISTING failing test on
  base `khoi`: `shipped_desired_state_file_is_valid` asserts the stale MCP server
  names `rcpa-user`/`dscc-user`/`biognosia-user`, but commit e597a99d8 renamed the
  shipped `config/desired-state.yaml` to `rcpa`/`dscc`/`biognosia` without updating
  this unit test (confirmed red: "rcpa-user missing from the file"). Since TEST-8
  extends exactly this test and it must be green for phase 8, aligned the asserted
  names to the shipped file (a trivially-correct, in-scope fix of the file I'm
  editing) and added the google assertions. Recorded here + surfaced in
  HUMAN_FEEDBACK as an incidental fix (not scope creep — it's a one-line alignment
  in the same test function this feature extends).

- **DRIFT-1.3** — verdict: none — schema (ITEM-1), `plan_auth_provider` (ITEM-2),
  `reconcile_auth_provider` + dispatch (ITEM-3), config yaml (ITEM-4), compose +
  README (ITEM-5) all implemented as planned. Absent-row → skip (never create),
  ensure → no-op, enforce → stamp-via-repository-encrypt, all as DECISIONS specify.

**Unresolved drifts:** 0
