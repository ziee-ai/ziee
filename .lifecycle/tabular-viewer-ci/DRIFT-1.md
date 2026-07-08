# DRIFT-1 — implementation vs plan

Round 1: audited the implemented diff against PLAN.md (ITEM-1..8). Divergences below.

- **DRIFT-1.1** — verdict: resolved — Plan mentioned "shared helpers exportTabularView/copyTabularSelection"; implementation additionally extracted a PURE `tabularClipboardText(view)` so the selection-vs-fallback decision is unit-testable without a DOM. Refinement in the plan's spirit (unit-testable helpers); PLAN.md ITEM-1 + TESTS.md TEST-U1 updated to name it. No behavior change.
- **DRIFT-1.2** — verdict: resolved — `testIds.generated.ts` (new testids) and the gallery state-matrix generated file were regenerated via `npm run gen:testid-registry` / `gen:state-matrix` to satisfy `npm run check`. Expected generated-artifact churn from ITEM-5/ITEM-7; added to PLAN.md "Files to touch".
- **DRIFT-1.3** — verdict: none — `DelimitedTable`'s `activeColumns` was converted from a plain arrow to `useCallback` so `publishView`'s dependency is stable (avoids the biome exhaustive-deps warning). Mechanical, matches the plan's publish-on-change intent.
- **DRIFT-1.4** — verdict: none — updated the `seeded-delimited-viewer` surface note (dropped the now-inaccurate "export/copy" words for the bare demo). Doc-only.

All divergences reconciled (plan amended where the impl was the better choice; no
`plan-wins` re-implementation needed).

**Unresolved drifts:** 0
