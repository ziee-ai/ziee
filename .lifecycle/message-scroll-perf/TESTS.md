# message-scroll-perf — TESTS

Every ITEM is covered by ≥1 TEST. The diff touches `src-app/ui/**` (real source),
so ≥1 `tier: e2e` test is required — satisfied by TEST-6..TEST-11. No cosmetic
tests: the e2e specs drive the REAL virtualizer against real scroll geometry and
measure the actual regression signals (estimated-vs-measured total, reserved
image height, prepend no-teleport); only the message payload + image bytes are
mocked at the network boundary.

**Test-harness note (reconciled in DRIFT-1):** the repo's unit tier is Node's
built-in test runner (`node --test`, `*.test.ts`, pure logic — NO React
Testing Library / jsdom). Component RENDER behavior is therefore covered by e2e;
the unit tier covers the PURE helpers the render logic was refactored to expose
(`estimateMessageHeight`, `measuredHeightCache`, `reservedImageBox`,
`classifyImageSrc`, `anchorRestoreNeeded`).

## Unit (node:test, pure)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/core/utils/estimateMessageHeight.test.ts` — asserts: `estimateMessageHeight` returns a larger estimate for a message with a table / image / code / tool block than a short user turn; grows monotonically with text length up to a cap; is null-safe (undefined/empty → the floor, never throws); narrower width ≥ wider width; and lands within a bounded ratio of representative real heights (RC1).
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.test.ts` — asserts: the width-bucketed cache stores/reads by `(id, widthBucket)`; misses at a different bucket (stale-width guard); hits within a bucket for sub-bucket jitter; `recordMeasurements` folds the virtualizer's size map (skipping numeric fallback keys); `buildInitialMeasurementsCache` emits `{key,size,index,start,end,lane}` entries ONLY for ids with a cached height at that bucket (ITEM-2).
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/reservedImageBox.test.ts` — asserts: `reservedImageBox` returns an exact `aspect-ratio` box when intrinsic dims are present (stable before AND after load), a `min-height` reservation when dims are absent and not loaded, and RELEASES the reservation once loaded; partial/non-positive dims fall back to the reservation (RC2/ITEM-3).
- **TEST-4** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/imageSrcPolicy.test.ts` — asserts: `classifyImageSrc` (the anti-exfil policy extracted from the `img` override) blocks external, `data:`, protocol-relative (`//host`), opaque-scheme (`javascript:`), and malformed src while allowing root-relative and same-origin — a regression guard that ITEM-3 did NOT weaken the image SSRF/exfil block (and closed the latent protocol-relative hole) (ITEM-3 security CONCERN).
- **TEST-5** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.test.ts` — asserts: `anchorRestoreNeeded` skips a restore already pinned within tolerance and is idempotent applied on top of a virtualizer scroll adjustment of the same delta (no double-count), honoring a custom tolerance; `indexRestoreOffset` still re-pins + clamps ≥ 0 (RC4/ITEM-6).

## E2E (Playwright)

- **TEST-6** (tier: e2e) [covers: ITEM-1, ITEM-5] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: on a 30-message MIXED window (short turns + long answers + tables + code), the INITIAL virtualizer total height (mostly estimated) is within ~35% of the FINAL total (all measured) — a ratio the old constant-140 estimate could not reach on heavy content (it undershot 3–4×) — proving the scrollbar thumb no longer lurches; and the mounted `chat-message` count stays bounded (overscan tuned, ITEM-5).
- **TEST-7** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: after scrolling a conversation fully (populating the measured-height cache), navigating away (unmount → flush) and back yields a total height already close (>0.8×) to the measured total BEFORE any re-scroll — proving `initialMeasurementsCache` seeded persisted heights (ITEM-2).
- **TEST-8** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: scrolling a code-heavy (Shiki) conversation up and back emits ZERO console errors and re-mounts rows cleanly — locking the already-correct `memo` boundary (pure scrolling never triggers a re-render/re-highlight storm) (RC-mem/ITEM-7).
- **TEST-9** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/chat/message-scroll-image-stability.spec.ts` — asserts: with an inline image served over a DELAYED same-origin route, the `reserved-image` placeholder is present with a real (>200px) reserved height and no `data-loaded` BEFORE the bytes arrive, gains `data-loaded` after, and a reference row below it moves < 24px across the load — proving ITEM-3 removed the async-image re-measure jump (RC2, symptom 3).
- **TEST-10** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/chat/lazy-load-messages.spec.ts` — asserts: the existing prepend no-teleport invariant STILL holds after the anchor-reconcile guard — after older messages load on scroll-up, the viewport `scrollTop` grows by ~the prepended content height (no double-adjust jump). Regression gate for ITEM-6 (existing spec kept green).
- **TEST-11** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: a 100-row markdown table renders inside a height-capped box (near `min(60vh,36rem)` + chrome, NOT 100 rows tall) — so heavy inline content imposes a definite height and its inner ResizeObserver can't feed back into row measurement (RC3/ITEM-4).

## Regression guards kept green (not new, but must pass in phase 8)

- `virtualize-messages.spec.ts` — virtualization still reduces mounted rows.
- `lazy-load-jump-to-message.spec.ts` — jump-to-message still lands + centers.
- `conversation-find.spec.ts` — find still surfaces + jumps to a virtualized-out match.
- `inline-csv-height-stability.spec.ts` — CSV inline height still stable (ITEM-4 sibling).
- `markdown-rendering.spec.ts` — markdown (incl. images now via ReservedImage) still renders.

The phase-8 frontend gate additionally requires `npm run check (ui): PASS`
(tsc + biome + lint:colors + kit-manifest + testid-registry + design-spec +
gallery-coverage + state-matrix) and the `gate:ui` runtime/visual pass on the
touched gallery surfaces.
