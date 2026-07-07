# TESTS — kit-table-actions

Test tiers mirror the repo: **unit** = `node --test "src/**/*.test.ts"` over the
pure, React-free `table-view-core.ts` + viewer `tableView.ts`; **e2e** = Playwright
against the **backend-free gallery** (the deterministic surface the whole
visual-testing system runs on — `tests/e2e/visual/gallery.spec.ts` pattern). Kit
capabilities, the tabular viewer, and both grids are driven through gallery story
cases / seeded surfaces added in ITEM-19, so no full-stack login/upload is needed.

Bipartite: every ITEM-1..19 is covered by ≥1 TEST; each TEST names a valid ITEM.
Frontend diff ⇒ ≥1 `tier: e2e` (there are 15).

## Unit (pure core + export helpers)

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: default view state (no sort/filter/hidden) returns dataSource unchanged in original order; `deriveView` is a pure identity when no ops are set (backward-compat baseline for the new props)
- **TEST-2** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `compareValues` orders numbers numerically (2 < 10) and non-numeric via locale string compare; a custom `sorter` overrides the default
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `applySort` tri-state — asc then desc reverse each other and `none` restores the original dataSource order (stable)
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `applyFilter` keeps rows whose any visible-column cell text contains the query case-insensitively; empty query is a passthrough; a query matching nothing yields `[]`
- **TEST-5** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `detectNumericColumns` marks a column numeric only when ALL sampled non-empty values parse as finite numbers; empties are ignored; a mixed column is not numeric; sampling caps at 50 rows
- **TEST-6** (tier: unit) [covers: ITEM-2, ITEM-10] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `deriveView` applies filter BEFORE sort and returns the same reference-stable rows so `onViewChange` consumers get filtered+sorted order; numeric flags computed once
- **TEST-7** (tier: unit) [covers: ITEM-9] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `serializeSelectionTsv` renders a single cell as its value, a row selection as tab-joined visible cells, and a multi-row range as newline-joined TSV (in view order)
- **TEST-8** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `clampWidth` never returns below `minWidth` (default 64) and respects an explicit per-column `minWidth`
- **TEST-9** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/components/ui/kit/table-view-core.test.ts` — asserts: `canHideColumn` returns false when hiding would leave zero visible columns (last-visible guard) and true otherwise
- **TEST-10** (tier: unit) [covers: ITEM-13] file: `src-app/ui/src/modules/file/viewers/tabular/tableView.test.ts` — asserts: `rowsToDelimited` serialises the current view to CSV/TSV honouring the active delimiter, RFC-4180 quoting (embedded quote/comma/newline), sorted+filtered order, and hidden-column exclusion
- **TEST-11** (tier: unit) [covers: ITEM-13, ITEM-15] file: `src-app/ui/src/modules/file/viewers/tabular/tableView.test.ts` — asserts: the copy/export helper excludes the `#` row-number gutter from exported data columns and preserves the original row identity when copying a selection

## e2e — kit `<Table>` capabilities (gallery story cases)

- **TEST-12** (tier: e2e) [covers: ITEM-1, ITEM-19] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: the unchanged "basic" Table gallery case still renders its rows (no regression from the new optional props) and the new capability story cases mount with zero console errors
- **TEST-13** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: clicking a sortable header cycles row order none→asc→desc→none and sets `aria-sort` on the `<th>` accordingly
- **TEST-14** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: typing in the toolbar search input filters visible rows case-insensitively; a no-match query shows the empty slot; clearing restores all rows
- **TEST-15** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: dragging a column's resize handle changes that column's rendered width; double-click resets it
- **TEST-16** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: opening the column-chooser and unchecking a column hides its cells; re-checking restores them; the last visible column's toggle is disabled
- **TEST-17** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: an auto-detected numeric column renders right-aligned with `font-variant-numeric: tabular-nums` (computed style) while a text column stays left-aligned
- **TEST-18** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: an `ellipsis` cell whose text overflows is single-line truncated and carries a `title` attribute equal to the full cell text
- **TEST-19** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: clicking a cell selects it (`data-selected` + ring); clicking a row-header selects the row; Ctrl/Cmd+C writes the selection as TSV to the clipboard (read back via the Clipboard API)
- **TEST-20** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/visual/kit-table-capabilities.spec.ts` — asserts: setting `scrollToIndex` scrolls a virtualized gallery Table so the target row becomes visible (jump mechanic underpinning ITEM-14)

## e2e — tabular file viewer (gallery seeded DelimitedTable + XlsxBody)

- **TEST-21** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: on a seeded CSV surface, sorting a column reorders rows and the search box filters them; the `#` gutter stays as the first (numeric, non-hideable) column
- **TEST-22** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: on a seeded XLSX surface, a sheet's table exposes the same sort + filter controls and reorders/filters rows
- **TEST-23** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: after filtering, "Export view" triggers a download whose CSV payload contains only the filtered/sorted rows (verified via the download event / captured blob)
- **TEST-24** (tier: e2e) [covers: ITEM-14] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: the readout shows "Showing X of Y rows" (updating as the filter changes) and entering a row number in jump-to-row scrolls that original row into view
- **TEST-25** (tier: e2e) [covers: ITEM-15] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: selecting a cell/row and clicking the viewer "Copy" button writes the selection as TSV to the clipboard (whole view when nothing is selected)
- **TEST-26** (tier: e2e) [covers: ITEM-16] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: clicking a clipped (ellipsis) cell opens a popover showing the full cell value

## e2e — data grids (gallery seeded)

- **TEST-27** (tier: e2e) [covers: ITEM-17] file: `src-app/ui/tests/e2e/visual/data-grids.spec.ts` — asserts: the MCP tool-calls grid sorts by the Duration column over the loaded page (server-paginated → sort-only, no client-side filter per DEC-5); the Duration column is right-aligned (numeric)
- **TEST-28** (tier: e2e) [covers: ITEM-18] file: `src-app/ui/tests/e2e/visual/data-grids.spec.ts` — asserts: the memory audit-log grid sorts by a column and filters rows via the search box (over the ≤limit loaded rows)
