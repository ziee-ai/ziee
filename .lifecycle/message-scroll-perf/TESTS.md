# message-scroll-perf — TESTS

Every ITEM is covered by ≥1 TEST. The diff touches `src-app/ui/**` (real source),
so ≥1 `tier: e2e` test is required — satisfied by TEST-7..TEST-11. No cosmetic
tests: the e2e specs drive the REAL virtualizer against real scroll geometry and
measure the actual regression signals (thumb/geometry stability, no jump on image
load, no teleport on prepend); only the message payload + image bytes are mocked
at the network boundary.

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/core/utils/estimateMessageHeight.test.ts` — asserts: `estimateMessageHeight` returns a larger estimate for a message containing a markdown table / image / code fence than for a short user text turn; grows monotonically with text length up to its cap; is null-safe (returns the floor constant for an undefined/empty message); and its estimates land within a bounded ratio of representative real heights (fixture heights) — proving first-pass estimates track reality (RC1).
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.test.ts` — asserts: the width-bucketed cache stores/reads a height by `(messageId, widthBucket)`; a lookup at a DIFFERENT width bucket misses (stale-width guard); `buildInitialMeasurementsCache(ids, width)` emits entries only for ids with a cached height at that bucket and in the virtualizer's `{key,size,start,index}` shape; and a coarse width delta within the same bucket still hits (no thrash on sub-bucket resizes) (RC1/ITEM-2).
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/ReservedImage.test.tsx` — asserts: `ReservedImage` renders a box that reserves height BEFORE load (honors intrinsic `width`/`height` → aspect-ratio; else a stable min-height), passes through `src`/`alt`, and clears the reserved placeholder styling on `onLoad` — so the row height is stable from first paint (RC2).
- **TEST-4** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/core/utils/useStreamdownComponents.test.tsx` — asserts: the `img` override still returns `BlockedImage` for an external / `data:` src (security policy preserved) and now routes an ALLOWED (same-origin / `/`-rooted) src through `ReservedImage` rather than a bare `<img>` — regression guard that ITEM-3 did not weaken the image SSRF/exfil block (ITEM-3 CONCERN).
- **TEST-5** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/MarkdownTable.test.tsx` — asserts: `MarkdownTable` (and the inline-preview wrapper touched by ITEM-4) renders its body inside a DEFINITE-height container (the `max-h-[min(60vh,36rem)]` cap / fixed inline box), so an inner ResizeObserver cannot resolve height to `auto` and feed back into row measurement (RC3).
- **TEST-6** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.test.ts` — asserts: the anchor-restore math used by ITEM-6 (`indexRestoreOffset` / any new measured-offset helper) re-pins the anchor index at its captured viewport offset and is idempotent when applied on top of a virtualizer scroll adjustment of the same delta (no double-count), clamped ≥ 0 (RC4). Extends the existing file.

## E2E

- **TEST-7** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-5] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: on a 500-message MIXED conversation (short turns + long answers + markdown tables + code blocks), scrolling top→bottom in steps keeps the virtualizer's total scroll height (`scrollHeight`) and the scrollbar thumb geometry STABLE — the total-size never oscillates beyond a small bound per step (no jumpy thumb, RC1) — and the mounted `chat-message` count stays bounded (overscan tuned, RC5). Regression signal: the pre-fix constant estimate produces large per-step `scrollHeight` jumps; the fix keeps them small.
- **TEST-8** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: re-opening the SAME long conversation (navigate away + back) starts with rows at their persisted measured heights — the first scroll after re-open shows a materially smaller cumulative `scrollHeight` correction than a cold first-ever open — proving the measured-height cache seeds `initialMeasurementsCache` (ITEM-2).
- **TEST-9** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/chat/message-scroll-image-stability.spec.ts` — asserts: with an assistant message carrying inline images served over a DELAYED same-origin route, the row occupies its reserved height before the bytes arrive and the viewport content does NOT jump when each image finishes loading (a fixed reference row's `boundingBox().y` stays within a small tolerance across the load) — proving ITEM-3 removed the async-image re-measure thrash (RC2, symptom 3).
- **TEST-10** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/chat/lazy-load-messages.spec.ts` — asserts: the existing prepend no-teleport invariant STILL holds after the anchor reconcile — after older messages load on scroll-up, the viewport `scrollTop` grows by ~the prepended content height and the previously-top-visible message stays put (no double-adjust jump). Regression gate for ITEM-6 (extends/keeps the existing spec green).
- **TEST-11** (tier: e2e) [covers: ITEM-7, ITEM-1] file: `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — asserts: pure scrolling of a conversation containing a fenced code block (Shiki-highlighted) does NOT re-highlight / re-mount message bodies — a highlighted row scrolled out and back retains identity and the app emits no per-frame re-render churn (mounted-count stable, no console error), locking the memoization boundary (RC-mem).

## Regression guards kept green (not new, but must pass in phase 8)

- `virtualize-messages.spec.ts` — virtualization still reduces mounted rows.
- `lazy-load-jump-to-message.spec.ts` — jump-to-message still lands + centers.
- `conversation-find.spec.ts` — find still surfaces + jumps to a virtualized-out match.
- `inline-csv-height-stability.spec.ts` — CSV inline height still stable (ITEM-4 sibling).

The phase-8 frontend gate additionally requires `npm run check (ui): PASS`
(tsc + biome + lint:colors + kit-manifest + testid-registry + design-spec +
gallery-coverage + state-matrix) and the `gate:ui` runtime/visual pass on the
touched gallery surfaces.
