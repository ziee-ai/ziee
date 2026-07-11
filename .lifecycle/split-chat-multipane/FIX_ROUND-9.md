# FIX_ROUND-9 — split-chat-multipane (iteration round 2 blind audit)

Round-1 converged at FIX_ROUND-8 (0 new). This round is the blind multi-angle
audit of the round-2 DELTA only (ITEM-40/41/42), run on the merged base.

## Blind round (3 fresh agents, diff-only context: `git diff a768f2769~1...a768f2769`)

- **Agent A — correctness / concurrency / state-management** → **[]**. Did a
  byte-level comparison of every replaced `File.store` block (clearFiles,
  getFileIds, getFiles, isUploading, setBackupFiles, restoreFromBackup) against
  the extracted `composerOwnership.ts` helpers; confirmed identical semantics
  (ordering, owner deletion, merge-not-replace), immer-safety (inputs never
  mutated), consistent single-pane-key resolution, and a transparent re-export.
- **Agent B — patterns-conformance / error-handling / api-contract** → **[]**.
  Confirmed `composerOwnership.ts` follows the `approvalRouting.ts` precedent
  (zero imports → node:test-loadable), the six external `composerPaneKey`
  importers are unchanged, no dropped guard/log, and no return-shape change.
  Ran the unit suite: 16/16 pass.
- **Agent C — tests-quality / test-coverage** → **1 LOW**. Confirmed the e2e
  discriminators are genuine (send-button has no content-gating so
  `send1.toBeEnabled` + `send0.toBeDisabled` isolate the blocker; pane-scoped
  locators; `data-filename` distinguishes the files; merge-not-replace + immer
  asserted in the unit suite). Flagged that `coverageGapsDoc.test.ts` (TEST-62)
  was a **self-referential tautology** — asserting a same-commit markdown doc
  contained strings the same commit authored, exercising no product code.

## Fix

- **[C-1, LOW — coverageGapsDoc tautology]** Added a doc-vs-repo cross-validation
  test: it now asserts the artifacts `COVERAGE_GAPS.md` credits actually exist on
  disk with their load-bearing markers (`composerOwnership.ts` exporting
  `mergeOwnedInto`; `composer-files-per-pane.spec.ts` asserting
  `assistant-status-chip`), so the test FAILS if the impl/spec is deleted while
  the doc still points at it. `readFileSync` throws ENOENT (→ fail) on a bad path,
  never a silent pass.

**New confirmed findings:** 1
