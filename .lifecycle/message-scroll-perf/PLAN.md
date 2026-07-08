# message-scroll-perf — PLAN

A **perf-regression fix** (not a new feature) for the conversation message scroll,
introduced by the just-merged lazy-load + `@tanstack/react-virtual` virtualization
of `MessageList` (main `a101c851`). User-reported symptoms:

1. Scrolling the message list is laggy / janky.
2. The scrollbar thumb is jumpy (size/position jumps as you scroll).
3. Worst with complex variable-height rows — inline tables and images — laggy up
   AND down.

## Root-cause summary (from code analysis; validated by the enumerated tests)

- **RC1 — constant `estimateSize: () => 140`.** Real rows span ~60px (short user
  turn) to 1500px (long answer / capped table / image). Each unmeasured row that
  scrolls into view corrects `getTotalSize()`, which resizes the scroll container
  → the scrollbar thumb jumps (symptom 2), and dragging the thumb into unmeasured
  territory teleports (symptom 1). `MessageList.tsx:62,91`.
- **RC2 — markdown images render with no reserved height** (`useStreamdownComponents.tsx:141`
  `img → <img {...props}/>`). The row measures at ~0 image height, the image
  loads async, grows the row, the `measureElement` ResizeObserver fires →
  `resizeItem` shifts geometry → jump (symptom 3, images).
- **RC3 — heavy inline content can feed its own ResizeObserver back into row
  measurement.** Row wrappers carry `ref={virt.measureElement}` (a RO on every
  row); `MarkdownTable`/inline previews run their own OverlayScrollbars +
  RO. Micro-jitter from those churns `resizeItem`. The existing
  `inline-csv-height-stability.spec.ts` proves this class of bug already bit the
  CSV viewer; tables/images need the same definite-height guarantee.
- **RC4 — prepend anchor-restore vs the virtualizer's own estimate→measured
  self-correction.** On older-page prepend, `restoreAnchor` calls
  `virt.scrollToOffset(...)` computed from **estimated** offsets while the
  virtualizer *also* self-adjusts scroll as the prepended rows measure — the two
  can double-adjust into a visible jump. `MessageList.tsx:199-207`,
  `ConversationPage.tsx:367-375`.
- **RC5 — flat `overscan: 8`.** With wildly-variable heavy rows, 8 rows each
  direction mounts up to ~16 heavy off-screen rows (tables/images) → extra
  measure+paint during scroll. `MessageList.tsx:92`.
- **RC-mem (REJECTED as a cause, kept as a guard).** `ChatMessage` is already
  `memo`-wrapped and its props (`message`, `isStreaming`) + consumed context
  (`useConversationFind`) are stable during pure scroll, so scrolling does NOT
  re-render / re-highlight message bodies. The "Shiki re-highlights on every
  windowing pass" hypothesis is false; ITEM-7 locks it with a regression test.

## Items

- **ITEM-1**: Replace the constant `estimateSize: () => 140` with a content-aware
  per-message estimator `estimateMessageHeight(msg, width)` (pure function) that
  inspects each message's content blocks (role, text length, presence of a
  markdown table, image, code fence, tool_use/tool_result) and returns a
  first-pass height within ~1.5× of reality instead of 5–10× off — shrinking
  `getTotalSize()` corrections so the scrollbar thumb stays stable (RC1 →
  symptoms 1,2).
- **ITEM-2**: Persist measured row heights across remount. A width-bucketed
  module-level `Map<messageId, height>` is fed from the virtualizer's
  measurements and seeded into `useVirtualizer({ initialMeasurementsCache })`, so
  re-opening / re-mounting a long conversation starts rows at their true measured
  height (zero first-scroll correction). Bucketed on a coarse viewport-width key
  so a resize invalidates stale-width heights rather than restoring wrong ones
  (RC1 → symptom 2).
- **ITEM-3**: Reserve image height so async image load can't thrash measurement.
  A `ReservedImage` wrapper for the markdown `img` renderer honors intrinsic
  `width`/`height` when present, else reserves a stable min-height/aspect-ratio
  box, settling on `onLoad`. Removes the estimate→actual delta cascade for image
  rows (RC2 → symptom 3, images).
- **ITEM-4**: Give heavy inline content a **definite** height so its inner
  ResizeObserver cannot feed back into the row's measured height (generalize the
  CSV height-stability guarantee). Verify `MarkdownTable` (already
  `max-h-[min(60vh,36rem)]`) and inline file/image previews all impose a definite
  box; add the missing bound where absent (RC3 → symptoms 1,3).
- **ITEM-5**: Tune `overscan` from a flat 8 to a value fit for variable heavy
  rows (fewer heavy off-screen mounts, pop-in still acceptable), value locked in
  DECISIONS by measurement (RC5 → symptoms 1,3).
- **ITEM-6**: Reconcile the prepend scroll-anchor restore with the virtualizer's
  own estimate→measured self-correction so an older-page load doesn't
  double-adjust into a jump — restore the anchor from measured offsets (or drop
  the redundant manual `scrollToOffset` when the virtualizer's built-in
  above-viewport adjustment already pins it), keeping the existing no-teleport
  invariant (RC4 → symptom 1 on prepend).
- **ITEM-7**: Lock the memoization boundary: pure scrolling must never
  re-render / re-highlight a message body. Fix only if a real invalidation is
  found (e.g. an unstable find-context value or the global `isStreaming`
  subscription rippling to non-streaming rows); otherwise this is a
  regression-guard test over the already-correct `memo` boundary (RC-mem →
  symptom 1).

## Files to touch

- `src-app/ui/src/modules/chat/components/MessageList.tsx` — estimateSize,
  initialMeasurementsCache wiring, measured-cache write-back, overscan, anchor
  reconcile (ITEM-1,2,5,6).
- `src-app/ui/src/modules/chat/core/utils/estimateMessageHeight.ts` — **new**,
  pure estimator (ITEM-1).
- `src-app/ui/src/modules/chat/core/utils/estimateMessageHeight.test.ts` —
  **new**, unit (ITEM-1).
- `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.ts` — **new**,
  width-bucketed module cache (ITEM-2).
- `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.test.ts` — **new**,
  unit (ITEM-2).
- `src-app/ui/src/components/common/ReservedImage.tsx` — **new**, height-reserving
  image (ITEM-3).
- `src-app/ui/src/components/common/reservedImageBox.ts` (+ `.test.ts`) — **new**,
  pure reservation helper split out for unit testing (ITEM-3; see DRIFT-1).
- `src-app/ui/src/components/common/imageSrcPolicy.ts` (+ `.test.ts`) — **new**,
  pure anti-exfil classifier extracted from the `img` override (ITEM-3; DRIFT-1).
- `src-app/ui/src/modules/chat/core/utils/useStreamdownComponents.tsx` — route the
  `img` override through `classifyImageSrc` + `ReservedImage` (ITEM-3).
- `src-app/ui/src/modules/chat/components/MessageList.tsx` +
  `core/utils/scrollAnchor.utils.ts` (+ `.test.ts`) — anchor-reconcile guard
  (ITEM-6).
- `src-app/ui/src/components/common/MarkdownTable.tsx` — **NOT touched**: ITEM-4 is
  verification-only (the table already caps at `max-h-[min(60vh,36rem)]` and
  inline previews at `max-h-[…]`) — see DRIFT-1.
- `src-app/ui/tests/e2e/chat/message-scroll-perf.spec.ts` — **new**, e2e perf
  regression (ITEM-1,2,4,5,7).
- `src-app/ui/tests/e2e/chat/message-scroll-image-stability.spec.ts` — **new**,
  e2e image-load stability (ITEM-3).
- Gallery cassette/state additions only if `check:state-matrix` demands a cell for
  `ReservedImage`'s placeholder state (ITEM-3, DEC-9) — else none.

Desktop (`src-app/desktop/ui`) consumes `../../ui/src` for the chat module via
its `@/` fallback alias (no local override), so editing `src-app/ui/**` covers
both UI surfaces; the diff stays in the `ui` workspace.

## Patterns to follow

- **Virtualizer setup**: mirror the existing `MessageList.tsx` `useVirtualizer`
  block and `kit/table.tsx`'s scroll-ready pattern (already referenced in the
  file) — extend it, don't restructure.
- **Pure util + colocated `.test.ts`**: mirror
  `core/utils/scrollAnchor.utils.ts` (+ `.test.ts`) and
  `core/stores/messageWindow.ts` (+ `.test.ts`) — the established chat pure-helper
  pattern with unit tests.
- **Height-reserving / definite-height inline content**: mirror the fix behind
  `inline-csv-height-stability.spec.ts` (the `DelimitedTable`/`inlineFill`
  definite-height box) for ITEM-3/ITEM-4.
- **e2e long-conversation harness**: mirror `virtualize-messages.spec.ts` +
  `helpers/sse-mock-helpers.ts` (`mockPaginatedMessages`, `mockUserMessage`,
  `MockMessageWithContent`) for seeding a 500-message mixed conversation, and
  `inline-csv-height-stability.spec.ts` for the height/stability assertion style.
- **Markdown component overrides**: mirror the existing `img`/`table` overrides in
  `useStreamdownComponents.tsx` and the `BlockedImage` component for
  `ReservedImage`.
