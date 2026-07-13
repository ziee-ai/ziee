# FIX_ROUND-2 — remediation of the round-1 re-audit

The blind re-audit after FIX_ROUND-1 (2 fresh diff-only agents on the reworked
store + widget) surfaced TWO new interaction bugs introduced/exposed by the
round-1 fixes. Both fixed:

- **HIGH — auto-load failure loop** (widget correctness): the last-virtual-item
  auto-load effect had no failure gate. On a persistent load-MORE failure the
  store leaves `recentHasMore=true` and only toggles `recentLoadingMore`, so the
  effect re-fired the instant it flipped false → a tight infinite `loadMoreRecent`
  loop hammering the API. (This path pre-existed but became reachable/visible once
  paging worked.) Fixed: the effect now also gates on `!recentError` (a failed
  loadMore sets `recentError`), and scrolling away from the bottom clears it via
  the new `clearRecentError` action so returning retries once. The empty-list
  first-load error keeps its ErrorState retry button. Covered by **TEST-13** (e2e:
  every next-page forced to 500 → bounded request count).

- **MED — syncRecentFront cursor divergence** (store state-management): unlike the
  delete paths, `syncRecentFront` merge-prepended new rows without re-anchoring
  `recentPage`. Once accumulated cross-device prepends reached `limit`, the next
  `loadMoreRecent` fetched a fully-overlapping page → `added===0` → the new
  no-progress guard wrongly stopped paging, stranding older rows. Fixed:
  `syncRecentFront` now re-anchors `recentPage=floor(length/limit)` like the delete
  paths. Covered by **TEST-5c** (unit: paging keeps progressing after a full-page
  prepend).

## Verification

- `npm run check` (ui): PASS. `tsc`: clean. Unit tests: 12/12 PASS.
- `gate:ui --skip-visual`: PASS (exit 0) — runtime-health boot canary green for the
  new loaded / loading-more / error seeds.

**New confirmed findings:** 3 (the round-2 re-audit surfaced three more delete/
failure edge cases — see FIX_ROUND-3)
