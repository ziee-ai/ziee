# TESTS — tabular-viewer-ci

Every ITEM is covered by ≥1 TEST; the UI flow is covered by e2e specs. The tabular
serialization/selection logic keeps its pure unit coverage; the header→body hookup is
proven end-to-end through the header-inclusive gallery surface.

- **TEST-U2** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/file/viewers/tabular/tableView.test.ts` — asserts: the `rowsToDelimited`/`exportFilename` serializers that `exportTabularView` reuses honour the delimiter, RFC-4180 quoting, the visible-column subset (hidden-column exclusion), formula-neutralization, and the `-view` filename. (`exportTabularView`/`copyTabularSelection` are thin DOM wrappers over these serializers — no independent DOM unit test; the wiring is proven by the e2e specs below.)
- **TEST-23** (tier: e2e) [covers: ITEM-2, ITEM-3, ITEM-5, ITEM-6, ITEM-7, ITEM-8] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: on the header-inclusive surface, filtering to "Banana" then clicking the header Export-view button downloads `data-view.csv` containing only the header + Banana row (not Cherry/apple).
- **TEST-25** (tier: e2e) [covers: ITEM-2, ITEM-3, ITEM-5, ITEM-6, ITEM-7, ITEM-8] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: selecting the row-0 Name cell then clicking the header Copy-selection button writes `Banana` to the clipboard as TSV.
- **TEST-24** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts` — asserts: the tabular toolbar (readout + jump-to-row) still renders/behaves after `TabularToolbar`'s prop change (`Showing X of Y rows`, jump scrolls the target row into view).
- **TEST-CHECK** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/file/viewers/tabular/TabularToolbar.tsx` — asserts: `npm run check` (tsc) type-checks the optional-prop signature with both callers (`DelimitedTable` omitting, `XlsxBody` passing).
