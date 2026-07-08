# FIX_ROUND-1 — tabular-viewer-ci

## Fixes applied (all CONFIRMED findings from the round-1 blind audit — LEDGER.jsonl)

- **F1** (correctness) — DelimitedTable's mount/data effect now resets
  `viewRef`/`visibleKeysRef`/`selectionRef` to the fresh `dataSource` before publishing
  (no stale snapshot on a file/text change).
- **F2** (state-management / regressions) — added `clearFileTabularView` + a DelimitedTable
  unmount cleanup effect (header actions disable instead of acting on a no-longer-rendered
  view).
- **F3** (patterns) — Copy-selection warns `Select a cell to copy` and copies nothing on an
  empty selection (removed the whole-view fallback; `tabularClipboardText` deleted).
- **F4** (i18n) — success toast is now `Copied selection` (matches chrome.tsx).
- **F5** (patterns) — Export-view uses the distinct `FileDown` glyph (vs the shell's
  `Download` tray beside it).
- **F6** (i18n) — Export tooltip/aria-label is format-aware: `Export view (CSV)` / `(TSV)`.
- **F7** (error-handling) — `onExport` wrapped in try/catch → `Failed to export` toast.
- **F8** (patterns/naming) — testid renamed to `file-viewer-tabular-copy-selection-btn`
  (spec + generated registry updated); added a visible-column-subset unit assertion.

Rejected round-1 findings (rationale recorded in LEDGER.jsonl `status:rejected`): the
header re-render / publish-churn perf & concurrency items (idiomatic, negligible, the read
is also needed to retitle the export label) and the disabled-state / clipboard-failure test
gaps (transient mount state / not browser-inducible).

## Re-audit (blind, full diff `main...HEAD`) — 2 NEW confirmed low findings

- **NEW-1** (state-management) — F1's reset effect keyed on `[publishView, dataSource,
  exportColumns]`; because `publishView` depends on `fileName`, a rename (fileName changes
  while `text` is unchanged) re-fires the reset and clobbers the user's live filter/selection.
- **NEW-2** (tests-quality) — the visible-column-subset unit test added in F8 duplicates the
  existing exclusion test (line 37) and never invokes the new DOM helpers, so it is cosmetic.

**New confirmed findings:** 2
