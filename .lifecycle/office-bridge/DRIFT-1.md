# DRIFT-1 — office-bridge (consolidated)

Implementation-vs-plan reconciliation, consolidated across the five stages. Each
stage ran its own drift loop to convergence during its build (the per-stage
DRIFT files are preserved in branch history); the material divergences that
shaped the shipped code are summarized here, all already reconciled.

- **DRIFT-1.1** — verdict: impl-wins — Windows live pane verification became a
  manual `WINDOWS_PANE_VERIFICATION.md` checklist rather than a `#[cfg(windows)]
  #[ignore]` cargo test: a real WebView2 pane can't be driven from cargo, exactly
  as the Mac verification is a manual doc. The cross-platform *backend* is proven
  by the non-cfg-gated `pane_rpc_test.rs`. PLAN amended accordingly.
- **DRIFT-1.2** — verdict: impl-wins — the WSS socket loop uses a `tokio::select!`
  duplex instead of a split-sink + writer-task; same effect, no `futures` split
  dependency. Frontend module discovery is glob-driven, so the server→desktop
  relocation needed no registry edits.
- **DRIFT-1.3** — verdict: plan-wins — the mode-gated approval decision was
  extracted to the pure `compute_needs_approval` (server `office_approval.rs`) so
  the office read-bypass is unit-testable AND the loop stays behaviour-preserving;
  the plan's split was implemented as written.
- **DRIFT-1.4** — verdict: resolved — the office approval path gained real
  integration coverage (`office_approval_test.rs` + `mock_office_server.rs`) after
  an initial (wrong) "not feasible in the server harness" note; a mock MCP server
  under the deterministic office id drives the real read-bypass / write-approval
  loop. The earlier note is superseded.
- **DRIFT-1.5** — verdict: resolved — the consolidated-lifecycle re-audit surfaced
  8 findings in the (relocated) code; 7 fixed (cert-staging TOCTOU on both
  platforms, a dead test, two stale migration cross-refs, plus a leak in the fix
  itself caught by the round-2 re-audit), 1 rejected with rationale (see LEDGER +
  FIX_ROUND-1).

**Unresolved drifts:** 0
