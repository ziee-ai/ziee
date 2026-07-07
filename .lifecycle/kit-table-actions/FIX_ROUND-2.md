# FIX_ROUND-2 — fix the 3 round-1 re-audit findings, then re-audit

## Fixed (the 3 confirmed findings from FIX_ROUND-1's re-audit)

- **A** — Export/Copy now honour the column-chooser: the kit's `onViewChange`
  reports the currently-visible non-gutter column keys (`{ visibleColumns }`);
  DelimitedTable/XlsxSheet build `activeColumns()` from them, so hidden columns
  drop out of both CSV/XLSX export and the whole-view Copy fallback (filtered +
  sorted order preserved via `viewRef`).
- **B** — Clipboard formula-injection closed on BOTH copy channels: a
  `sanitizeClipboard` prop threads a `sanitize` flag through
  `serializeTsv`/`serializeSelectionTsv` (new generic `neutralizeSpreadsheetCell`
  in the core), covering the kit Ctrl/Cmd+C path AND the viewer's Copy button
  (`selectionRef`, pre-neutralized by the kit). The file-export path already
  neutralized via `rowsToDelimited`.
- **C** — `McpToolCallsTab` no longer sets `filterable` (server-paginated →
  page-scoped filtering is misleading, DEC-5); it keeps `sortable` + a numeric
  Duration column. The non-paginated memory audit log retains sort + filter.

## Re-audit (fresh blind agent, full current diff, all angles)

The agent verified every round-2 fix is correct and complete (hidden-column
export/copy exclusion, dual-path clipboard sanitize, no MCP regression, no
render loop, `visibleKeysSig` non-issue). It found **one** genuine LOW defect:
`deriveView` filtered over `visibleColumns` which still included the `__rn`
row-number gutter, so a numeric query matched row numbers.

### Fixed (that finding)

`CoreColumn` gained an optional `rowHeader`; `deriveView` now filters over the
visible NON-gutter columns (sort still resolves against all visible columns).
Covered by a new unit test (`deriveView filter ignores the rowHeader gutter`).
The 2-line delta was self-audited across correctness / api-contract / perf /
security / state-management: additive optional field + one `.filter` on an
already-memoized derivation; existing callers + the `TableColumn`→`CoreColumn`
cast unaffected; unit + all 17 e2e green.

**New confirmed findings:** 0
