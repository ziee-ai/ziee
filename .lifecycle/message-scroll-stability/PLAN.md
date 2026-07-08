# PLAN — message-scroll-stability

## Root cause (measured / traced, not re-guessed)

Two independent defects share ONE mechanism: **a chat row's measured height keeps
changing AFTER it first mounts, and the row unmounts/remounts as it scrolls.**

`@tanstack/react-virtual` v3.14.5 rows use `ref={virt.measureElement}`. Every time
a rendered row's measured height differs from its cached size by any nonzero delta,
`virtual-core`'s `resizeItem` (traced in
`node_modules/@tanstack/virtual-core/dist/esm/index.js:826-883`):
1. bumps `itemSizeCacheVersion` → invalidates the memoized measurements → the next
   `getTotalSize()` returns a new value → the full-height spacer `div` changes size →
   **the OverlayScrollbars thumb resizes + repositions = the "jumps back and forth"**;
2. if the row is above the scroll offset (or it is a first measurement),
   `applyScrollAdjustment(delta)` shifts `scrollTop` programmatically → **a visible
   content jump**;
3. `notify(false)` → React re-render.

Row heights change post-mount continuously and unboundedly because inline content
settles asynchronously:
- **InlineFilePreview** (`modules/file/chat-extension/components/InlineFilePreview.tsx`)
  mounts its body **lazily** (`inView` via IntersectionObserver, `rootMargin:800px`,
  L62-83) — header-only height → header+body height is a LARGE delta that fires
  exactly while scrolling; and the body is **`max-h`-capped but content-driven**
  (L209-221), so an image decoding, a table/xlsx parsing, or markdown/Shiki
  highlighting each mutate the height again after mount.
- **CollapsibleBlock** (`modules/chat/components/CollapsibleBlock.tsx`) measures
  overflow via a ResizeObserver and its toggle mutates height.

The prior `message-scroll-perf` fix only shrank the **first** estimate→measure delta
(content-aware `estimateSize` + a seeded measured-height cache). It never stabilized
**post-mount** heights, so the recorrection storm — and the jitter — persists exactly
as reported. (A later commit even eased the scrollbar CSS "to absorb jitter" and was
reverted — a band-aid over this same root cause.)

**Show-more loss:** `collapsed` is `useState(true)` INSIDE `CollapsibleBlock`, which
lives inside `ChatMessage` — a virtualized row that UNMOUNTS beyond `overscan:8` and
remounts fresh → `collapsed` resets to `true`. `InlineFilePreview.collapsed`/`inView`
are the same class of per-mount `useState`. Scroll away and back → collapsed again;
and the remount re-runs the lazy body mount → re-feeds the height-churn every return.

The fix therefore has two levers: **(A) make inline heights stop changing after
mount** (fixed/reserved height + internal scroll), and **(B) lift the per-row
ephemeral UI state into a store keyed by a stable id** so unmount/remount is a
zero-delta, state-preserving no-op — plus **(C) serialize deliberate height changes
(expand / resize) with the anchor/reassert paths** so they grow in place instead of
fighting the virtualizer.

## Items

- **ITEM-1**: Add dev-only virtualizer-correction instrumentation to MessageList (count `itemSizeCache`-version bumps / total-size recomputes via the existing `onChange`, exposed on a `window.__MSGLIST_METRICS__` hook behind `import.meta.env.DEV`) plus a seeded ~500-message mixed-content (text, long-collapsible, table, image, inline-file-preview) gallery cell driving the real MessageList, so the root cause is empirically confirmed and the fix is gated by a measured near-zero correction count after scroll settle.
- **ITEM-2**: Give every inline file-view body a DEFINITE (reserved, capped) height with internal `overflow-auto` scroll in InlineFilePreview (replacing the content-driven `max-h`), and render a fixed-height skeleton for the `!inView` state at the SAME reserved height, so the lazy header-only→body transition and all async content settling (image decode, table/xlsx parse, Shiki) are zero-delta to the virtualized row height.
- **ITEM-3**: Add an optional bottom drag-resize handle to the inline file-view body so a user can grow/shrink one preview; the chosen height persists per file key (via ITEM-6) and survives remount, and a resize routes through the ITEM-7 in-place reconciliation (never a raw virtualizer recorrection).
- **ITEM-4**: Lift the show-more collapse state out of `CollapsibleBlock`/`ChatMessage` into the per-conversation view-state store keyed by message id, so expanding a long message survives virtualization unmount/remount (scroll away + back stays expanded).
- **ITEM-5**: Lift `InlineFilePreview`'s ephemeral state (collapsed, has-been-seen, resized-height) into the same store keyed by file key, so remount renders the body immediately at the same reserved height (zero delta, no re-fetch) and preserves the user's expand/collapse + resize choices.
- **ITEM-6**: Add the per-conversation `MessageViewState` store (`defineStore`, message-id→{collapsed} and file-key→{collapsed,seen,heightPx} maps), reset on conversation switch, mirroring `Chat.store`'s `conversationStateCache` lifecycle. Single source of truth for ITEM-3/4/5.
- **ITEM-7**: Expand/resize-in-place reconciliation in MessageList: expose an imperative "anchor a row's viewport-top across an intentional height change" method; `CollapsibleBlock`'s toggle and the ITEM-3 resize call it so the change grows DOWNWARD without the viewport jumping, serialized against (and cancelling) the reassert/anchor-restore paths so they don't double-adjust on the same frame.

## Files to touch

- `src-app/ui/src/modules/chat/core/stores/MessageViewState.store.ts` (NEW — ITEM-6 lifted view-state store)
- `src-app/ui/src/modules/chat/core/stores/messageViewState.helpers.ts` (NEW — pure key/reset helpers, unit-testable)
- `src-app/ui/src/modules/chat/components/CollapsibleBlock.tsx` (ITEM-4, ITEM-7: read/write collapsed from store; expand-in-place)
- `src-app/ui/src/modules/chat/components/ChatMessage.tsx` (ITEM-4: thread message id / collapse persistence)
- `src-app/ui/src/modules/chat/components/MessageList.tsx` (ITEM-1 instrumentation; ITEM-7 in-place anchor method)
- `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.ts` (ITEM-7: pure in-place-anchor helper)
- `src-app/ui/src/modules/file/chat-extension/components/InlineFilePreview.tsx` (ITEM-2 fixed height + skeleton; ITEM-3 resize handle; ITEM-5 lifted state)
- `src-app/ui/src/modules/file/chat-extension/components/inlineFileHeight.ts` (NEW — pure reserved-height + clamp helpers, unit-testable)
- `src-app/ui/src/modules/chat/core/stores/Chat.store.ts` (ITEM-6: reset MessageViewState on conversation switch, at the existing clear-window site)
- `src-app/ui/src/dev/gallery/seeded/shard5.tsx` (or a new shard) + `src-app/ui/src/dev/gallery/stateCoverage.ts` (ITEM-1: 500-msg mixed gallery cell + state registration)
- `src-app/ui/tests/e2e/…` chat scroll-stability spec (Phase 3/8)

## Patterns to follow

- **Lifted per-conversation view store (ITEM-6)** — mirror `Chat.store.ts`'s `conversationStateCache: Map<string, …>` + its clear-on-switch call site; author the new store with `defineStore` from `@/core/store-kit` exactly like the other chat-core stores (state maps + hook-free actions read via `Stores.X.$`). Reset hook wired where `Chat.store` already clears the message window.
- **Stable/reserved inline heights (ITEM-2)** — mirror the in-tree `file-viewer-virtualization` precedent: `RawCodeView.tsx`'s `contain-intrinsic-size` reserved chunks and `DelimitedTable.tsx`/`XlsxBody.tsx`'s definite-height (`h-[min(360px,55vh)]`) virtualized bodies; and `components/common/ReservedImage.tsx`'s reserve-then-release dims.
- **Drag-resize handle (ITEM-3)** — mirror the existing right-panel width-drag gesture (pointer-capture drag writing a persisted px into the store) used by the chat right panel; reuse the same pointer/gesture shape, not a new abstraction.
- **In-place anchor/measure (ITEM-7)** — mirror MessageList's existing `captureAnchor`/`restoreAnchor` + `startReassert`/`cancelReassert` and `scrollAnchor.utils.ts`'s pure `anchorRestoreNeeded`/`indexRestoreOffset`; add the new helper alongside them in the same pure module.
- **Gallery seeded cell + instrumentation (ITEM-1)** — mirror the existing `seededSurfaces.tsx`/`seeded/shard*.tsx` chat cells (`seeded-message-list-empty`, `seeded-chat-message-*`) and `stateCoverage.ts` registration; instrumentation guarded by `import.meta.env.DEV` like other dev-only gallery hooks.
