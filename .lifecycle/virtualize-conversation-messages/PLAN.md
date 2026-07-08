# PLAN — virtualize-conversation-messages

Row-virtualize the conversation message list so a long (lazy-loaded) window only
mounts the visible messages + overscan, not every loaded ChatMessage (each of
which renders markdown / katex / mermaid / shiki / tool-result cards / inline
file previews — expensive at 30–100+ loaded rows). Builds directly on the
lazy-load window from `feat/lazy-load-conversation-messages` (same branch); the
deferred "virtualization follow-up" noted there.

**Library: `@tanstack/react-virtual`** — already a dependency, already used 3× in
the repo (`kit/table.tsx`, `kit/tree.tsx`, `kit/multi-select.tsx`); `table.tsx`
is the closest idiom (dynamic `measureElement` for variable heights + OS-viewport
`getScrollElement` + `scrollToIndex` + `scrollerReady`). No new dependency.

**Load-bearing insight** (keeps the change focused): the virtualizer renders a
spacer of `getTotalSize()`, so the scroll **geometry (scrollHeight) is
preserved**. Therefore the lazy-load machinery I already built keeps working
unchanged: the top/bottom `IntersectionObserver` load-sentinels still fire
(they observe the real scroll height), and the initial bottom-jump + bottom-
follow (via `messagesEndRef`, a sibling below the spacer) still reach the true
bottom. Only two things genuinely break under virtualization and must change:

1. **Find / jump `scrollIntoView` on an off-screen message** — a loaded-but-
   virtualized-out message has no DOM node, so `document.querySelector(
   '[data-message-id]').scrollIntoView()` finds nothing. → use the virtualizer's
   `scrollToIndex(indexOfId)` instead.
2. **The element-measurement scroll-anchor on prepend** — after older messages
   prepend, the previously-top-visible message may be virtualized out before it
   can be re-measured. → anchor by INDEX via the virtualizer
   (`getOffsetForIndex` / `scrollToIndex`) instead of by `getBoundingClientRect`.

## Items

- **ITEM-1**: Virtualize the message rendering in `MessageList.tsx` with `useVirtualizer` (`@tanstack/react-virtual`): `count = messagesArray.length`, `getScrollElement` (the OverlayScrollbars viewport, provided by ConversationPage), **`getItemKey = i => messagesArray[i].id`** (stable keys so the measurement cache survives prepend/append/window-reset), `estimateSize` (~140px), `measureElement` ref on each row (variable heights), `overscan` (8). Render a container of `height: getTotalSize(); position: relative`; each `ChatMessage` in an absolutely-positioned wrapper at `transform: translateY(virtualItem.start)` carrying `data-index`, `data-message-id`, `data-role` + `ref={virt.measureElement}`. The non-message chrome (the `chat-loading-older` region, the streaming indicator, the `message_list_footer` slot, the empty-state) stay as NON-virtualized siblings above/below the virtual container. Keep the `isStreaming` last-assistant flag semantics.
- **ITEM-2**: Expose an imperative `scrollToMessageId(id, align?)` from `MessageList` via `forwardRef` + `useImperativeHandle` — maps `id → index` over the current window and calls `virt.scrollToIndex(index, { align })`; returns `false` when the id isn't in the loaded window (so callers can fall back to `jumpToMessage`). Also expose `measureFirstVisible()` / `restoreToIndex(index, viewportOffset)` for the index-based anchor (ITEM-4), or fold those into the same handle.
- **ITEM-3**: `ConversationPage.tsx` wiring: give `MessageList` the scroll element (pass `getScrollElement` derived from the existing `getViewport()`), hold a `messageListRef`, and route the `#message-<id>` deep-link jump through `jumpToMessage` (loads the around-window) → `messageListRef.current.scrollToMessageId(id, 'center')` → highlight, replacing the old `querySelector(...).scrollIntoView`. Keep the initial bottom-jump + bottom-follow (`messagesEndRef`) and the top/bottom `IntersectionObserver` load-sentinels unchanged (they work against the preserved scroll geometry).
- **ITEM-4**: Index-based prepend scroll-anchoring (replaces the element-measurement path for the virtualized list): before `loadOlderMessages`, capture `{ anchorId, viewportOffset }` where `anchorId` = the top-most rendered message and `viewportOffset` = its top relative to the viewport top; after the prepend renders, restore via the virtualizer — `scrollToOffset(getOffsetForIndex(newIndexOfAnchor, 'start')[0] − viewportOffset)` — so the previously-visible content stays put, reinforced by the virtualizer's `shouldAdjustScrollPositionOnItemSizeChange` (default on) settling estimate→measured height corrections. Keep the observer trigger + `pendingAnchorRef` lifecycle from lazy-load; only the capture/restore mechanism becomes index-based.
- **ITEM-5**: `ConversationFindBar.tsx`: `activateMatch` scrolls to a match via a `scrollToMessage(id)` callback (threaded from ConversationPage's `messageListRef`) — `scrollToMessageId` when loaded, else `jumpToMessage(id)` then `scrollToMessage(id)` — replacing `document.querySelector(...).scrollIntoView`. The results-list, "X of Y", pagination, and highlight are unchanged.
- **ITEM-6**: Gallery + state coverage for the virtualized `MessageList`: ensure the component-gallery renders the virtual container (the gallery mock viewport must give the scroller a real height so `useVirtualizer` measures), and register/allow-list any new conditional render state introduced by the virtual/empty branch so `check:state-matrix` + `check:gallery-coverage` pass.

## Files to touch

Frontend (chat module is `src-app/ui` only — it does NOT exist in `desktop/ui`):
- `src-app/ui/src/modules/chat/components/MessageList.tsx` (ITEM-1, ITEM-2 — virtualize + imperative handle)
- `src-app/ui/src/modules/chat/pages/ConversationPage.tsx` (ITEM-3, ITEM-4 — scroll-element wiring, index-based anchor, jump via scrollToIndex)
- `src-app/ui/src/modules/chat/components/ConversationFindBar.tsx` (ITEM-5 — scrollToMessage callback)
- `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.ts` (ITEM-4 — add/adjust pure index-anchor helpers; keep the existing pure fns used elsewhere)
- `src-app/ui/src/dev/gallery/**` (ITEM-6 — MessageList virtual render + any state cell)

No backend change → no migration, no OpenAPI regen. No `desktop/ui` change.

## Patterns to follow

- **Virtualizer** (ITEM-1/2/4): mirror `src/components/ui/kit/table.tsx` — `useVirtualizer({ count, getScrollElement, estimateSize, overscan })`, `virt.getVirtualItems()`, `virt.getTotalSize()`, `ref={virt.measureElement}` + `data-index` on each row, absolute `translateY(vi.start)`, and the `scrollToIndex(idx, { align })` effect. Reuse the `events={{ initialized: () => setScrollerReady(true) }}` scroller-ready pattern already in `ConversationPage`.
- **Imperative handle** (ITEM-2): standard React `forwardRef` + `useImperativeHandle` (see any kit component exposing a ref API).
- **Scroll-anchor helpers** (ITEM-4): extend `core/utils/scrollAnchor.utils.ts` (its pure `pickTopAnchor`/`restoreDelta` stay; add pure index-anchor math unit-tested like the existing `scrollAnchor.utils.test.ts`).
- **Find/jump highlight** (ITEM-3/5): reuse the existing `ConversationFindContext` + `data-find-active` + `jumpToMessage` (around=) flow from lazy-load; only the scroll call changes from `scrollIntoView` to `scrollToIndex`.
- **Gallery** (ITEM-6): mirror existing chat gallery entries under `src/dev/gallery/`.
