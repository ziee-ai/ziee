# DRIFT-3 — split-chat-multipane (iteration round 2)

Implementation-vs-plan reconciliation for the round-2 DELTA (ITEM-40/41/42), on
the current merged base (`origin/main`@304f4a011 merged into the branch). Round-1
items (1–39) already converged in DRIFT-1/DRIFT-2; this round reviews ONLY the
three new items.

- **DRIFT-3.1** — verdict: none — ITEM-40 (extract `composerOwnership.ts` +
  `File.store` delegates + `composerOwnership.test.ts`) implemented exactly as
  planned. Blind audit (2 fresh agents, correctness/concurrency/state +
  patterns/error/api) confirmed byte-identical behaviour vs the pre-refactor
  inline code and a transparent re-export; the six external `composerPaneKey`
  importers are unchanged. Reconciled.

- **DRIFT-3.2** — verdict: none — ITEM-41 (`composer-files-per-pane.spec.ts`,
  TEST-61) implemented as a 3-test spec (file attach/remove isolation;
  in-flight-upload send-blocker per-pane; assistant-chip isolation), each acting
  on a pane WITHOUT a prior focus-click (FB-4). Blind audit (tests-quality)
  confirmed the pane-scoped locators + cross-pane negative assertions are genuine
  discriminators (a shared-focused-buffer engine fails them). Reconciled.

- **DRIFT-3.3** — verdict: impl-wins — ITEM-42 shipped the durable
  `COVERAGE_GAPS.md` + `coverageGapsDoc.test.ts` (TEST-62) as planned, AND the
  test was strengthened beyond the plan: the blind audit flagged the original
  structural-only assertions as a tautology, so a doc-vs-repo cross-validation
  leg was added (asserts the artifacts the doc credits actually exist with their
  key markers). The plan said "structural check"; the impl does structural +
  consistency. No PLAN amendment needed — a superset of the planned assertion,
  recorded in FIX_ROUND-9. Reconciled.

**Unresolved drifts:** 0
