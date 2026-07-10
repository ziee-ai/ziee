# FIX_ROUND-1 — office-mode-gated-approval

## Fixes applied for the Phase-6 ledger (6 confirmed findings)

- **Perf — double server-id parse** (mcp.rs): parse `server_id` once into `server_uuid`,
  reused for both the `is_auto_approved` check and `compute_needs_approval`.
- **Perf — v5 hash per call** (office_approval.rs): `office_bridge_mcp_server_id()` now
  returns a cached `LazyLock<Uuid>`; `run_office_js_read_bypass` reordered to check
  `tool_name`/`mode` (cheap) BEFORE the server-id compare (short-circuits so a common
  non-`run_office_js` call never touches the id).
- **Tests — missing branches** (office_approval.rs): added a control-WRITE delegation
  assertion (`invoke_capability` of a mutating op → always-approve, even under
  AutoApprove) and a `disabled_arm_never_auto_runs` test — `compute_needs_approval`
  branch coverage is now complete (9/9).
- **Tests — load-bearing invariant** (office_approval.rs): new
  `office_bridge_is_not_approval_bypassed` asserts `office_bridge` is NOT in
  `is_builtin_server_id` (if it were, `compute_needs_approval` would short-circuit to
  `false` and every write would auto-run with no approval).
- **Patterns — stale comment** (handlers.rs): the `seeded_mock` doc no longer claims to
  exercise the removed PowerPoint capability branch.
- **Tests — stale name** (pane_rpc_test.rs): `test9_dispatch_tool_read_document_round_trip`
  → `..._run_office_js_...`.

Plus the two rejected patterns nits (LEDGER): the `office_bridge_server_id` vs
`office_bridge_mcp_server_id` naming (drift-test-locked; server name mirrors
`control_mcp_server_id`; a cross-reference doc comment was added) and the `is_control`
parameter (caller already computes it; passing avoids a per-call control-id hash).

Verified: server `office_approval::` unit tests → **10 passed, 0 failed** (incl. the 3
new tests); the whole change compiles.

## Re-audit round (2 blind reviewers on the fixed diff)

- Security/correctness re-audit: **clean** — the `&&` reorder is logically identical, the
  `LazyLock` cache is correct + thread-safe, parse-once preserves behaviour, and
  office_bridge is locked out of the bypass set. No new defect.
- Tests-quality re-audit: raised ONE HIGH ("`is_builtin_server_id` is private → E0603
  breaks the test build"). **FALSE POSITIVE, refuted:** `is_builtin_server_id` is
  `pub(crate)` (mcp.rs:292) — the reviewer inferred privacy from the diff, which does
  not include that line — and the test `office_bridge_is_not_approval_bypassed` in fact
  **compiled and passed** (server lib test run, log confirmed). The new tests are
  otherwise confirmed correct and the branch coverage complete.

**New confirmed findings:** 0
