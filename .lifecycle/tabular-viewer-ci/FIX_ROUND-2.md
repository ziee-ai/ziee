# FIX_ROUND-2 — tabular-viewer-ci

## Fixes applied (the 2 NEW findings from FIX_ROUND-1's re-audit)

- **NEW-1** (state-management) — re-keyed DelimitedTable's view-reset effect on
  `[dataSource]` only (with a `biome-ignore` + rationale), so a rename (fileName change
  with unchanged text) no longer re-fires it and clobbers the live filter/selection.
  `dataSource`/`exportColumns` are memoized on `[text, delimiter]`, so the effect still
  fires exactly on a new parse.
- **NEW-2** (tests-quality) — removed the redundant visible-column-subset unit test (the
  existing exclusion test already proves it; the DOM export/copy wrappers are proven by the
  e2e specs).

## Final blind re-audit (round 3, full diff `main...HEAD`) — result

The round-3 blind reviewer confirmed the implementation is correct: the `[dataSource]`
reset effect matches its comment, `clearFileTabularView` + the unmount cleanup are correct,
the `$` snapshot read is valid, the reactive Map read re-renders correctly, and there are
no dangling references (old `file-delimited-*` / `tabularClipboardText` testids/symbols
gone). The XLSX dead props are pre-existing on `main` and out of scope.

The one actionable item was a coverage gap on the new header branches. Closed it:
- **added TEST-27** (e2e) — Copy-selection with nothing selected warns and leaves the
  clipboard empty (locks the no-whole-view-fallback behavior).
- **added a TEST-23 assertion** on the format-aware export aria-label (`Export view (CSV)`).

These are test-only additions (no product code changed). The residual low branches —
disabled-until-published (transient synchronous-mount window) and clear-on-unmount (no
gallery unmount trigger for the CSV shell surface) and a clipboard-write failure (not
inducible headless) — are reasoned-rejected in LEDGER.jsonl.

No product code changed after the round-3 review that found the implementation correct, so
no further blind round is warranted.

**New confirmed findings:** 0
