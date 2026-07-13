# FIX_ROUND-4 — chats-page-virtualization (post-merge with live4 paging)

After merging `origin/main` (`e2b5bba3e`, live4's sidebar-recent infinite-paging
store), a blind audit vs the MERGED base confirmed the composition is correct
(the virtualizer reads the `/chats` `conversations`/`hasMore`/`total`/
`loadNextPage` fields and never the sidebar's `recent*` cursor; `loadNextPage`
still appends pages into `conversations`; `getItemKey` by id survives
append/prepend/delete/sync; hooks are unconditional). It found ONE real finding.

## Fixed

- **[MEDIUM] measured-height seed built at the fallback width** (state-management/
  perf). Unlike `MessageList` (which mounts at count 0, so its width layout-effect
  corrects `widthRef` before the count 0→N seed build), `VirtualizedConversationList`
  is parent-gated to mount at count>0, so its first render built + froze the
  `initialMeasurementsCache` seed at the DEFAULT (max) width BEFORE the
  width-measuring layout effect ran. virtual-core consumes the seed exactly once
  on its first `getMeasurements` (verified in `virtual-core` dist:
  `measurementsCache.length===0 → = initialMeasurementsCache`), during that first
  render — so at a sub-max-width desktop window the seed missed the cache bucket,
  silently defeating the ITEM-2 near-zero-re-open-correction optimization there
  (correctness + no-jank-at-rest were unaffected — the content-aware estimate
  still gave a good first pass). Fix: read the real content width SYNCHRONOUSLY
  ONCE at seed-build time (`readContentWidth()` — a single mount-time reflow, NOT
  the per-scroll reflow the ref/ResizeObserver dance avoids). The scroller is
  normally already initialized by the time count>0 (it mounts during the loading
  arm), so the seed now lands at the right width bucket; it falls back to the max
  width only if the scroller isn't ready yet. Verified: visual jank/window e2e
  (incl. the 390px narrow surface) still green.

- **[test-robustness] cold-start flake** — the visual spec's `beforeEach`
  "Showing 200 of 200" wait (30s) intermittently timed out on a heavily-loaded box
  during the cold gallery Vite boot (a different test each run; the app rendered
  fine). Bumped to 60s. Not masking a failure — a boot-timing budget.

## Re-audit

A FINAL full blind round (fresh diff-only agent over the merged+fixed diff) was
run. Result recorded below.

**New confirmed findings:** 0
