# PLAN_AUDIT — virtualize-conversation-messages

Audit of PLAN.md against the codebase (worktree on `feat/lazy-load-conversation-
messages` @ 8381bb27, the lazy-load tip = this feature's base).

## Breakage risk

- **Off-screen `[data-message-id]` nodes disappear** — the biggest behavioral
  change. Anything reaching for a message DOM node by id must tolerate it being
  virtualized out: (a) `ConversationFindBar.activateMatch` (`querySelector(...)
  .scrollIntoView`) and (b) `ConversationPage`'s `#message-<id>` hash handler.
  Both are converted to `scrollToMessageId` (ITEM-3/ITEM-5). The lazy-load e2e
  specs that assert `toHaveCount(0)` for UNLOADED messages still pass (unloaded ⇒
  not in the window ⇒ not rendered — same as before). Specs that assert a message
  is visible after a scroll/jump rely on the virtualizer rendering it: the anchor
  e2e (scrollTop/scrollHeight invariant) and jump/branch/find specs are the
  regression guard and MUST stay green post-virtualization.
- **Flex `gap-1` between rows is lost under absolute positioning** — the current
  list uses a flex column with `gap-1`; virtual rows are `position:absolute` so
  the gap won't apply. The per-row spacing must move INTO the measured row
  (padding/margin on the row wrapper) so `measureElement` includes it, else rows
  visually abut. Called out in ITEM-1.
- **Initial bottom-jump lands on ESTIMATES** — on first load the virtualizer has
  only `estimateSize` (rows not yet measured), so `getTotalSize()` (hence the
  `messagesEndRef` position) is estimated; `scrollIntoView(messagesEndRef)` then
  lands near — not exactly — the bottom until rows measure. Mitigation: keep the
  existing `isAtBottom` follow (re-scrolls as measurement settles) and/or re-jump
  once measured. Flagged on ITEM-3.
- **Streaming re-measures the last row every token** — `measureElement`'s
  ResizeObserver re-fires as the streaming assistant row grows; the virtualizer
  handles it, but it must not thrash the bottom-follow. The `!hasMoreAfter` +
  `isAtBottom` gate already limits follow to the tail; acceptable. Flagged on ITEM-1.
- **ChatMessage internals unaffected** — `ChatMessage` already carries
  `data-message-id`/`data-role`/`data-testid=chat-message` and reads the find
  highlight from `ConversationFindContext`; wrapping it in an absolutely-
  positioned measured row does not change its own layout. ✔ (grepped: no code
  assumes ChatMessage is a direct flex child.)

## Pattern conformance

- ITEM-1/2/4 mirror `kit/table.tsx`'s `useVirtualizer` + `measureElement` +
  `getScrollElement` + `scrollToIndex` + `scrollerReady` idiom exactly (the one
  repo reference doing dynamic variable-height virtualization). ✔
- ITEM-3/5 reuse the lazy-load `jumpToMessage` (around=) + `ConversationFindContext`
  + `data-find-active` flow; only the scroll call changes. ✔
- The `overflow-anchor:none` + `getViewport()` + `events.initialized` scroller
  plumbing from lazy-load is reused for `getScrollElement`. ✔

## Migration collisions

- **None.** Pure frontend; no `migrations/` change. Latest migration on disk
  unchanged (132). ✔

## OpenAPI regen

- **None.** No request/response type change; no new endpoint. The generated
  `openapi.json`/`api-client/types.ts` are untouched, so this stays a UI-only
  diff (no backend gate). `desktop/ui` unaffected — the chat module does not
  exist there. ✔

## Per-item verdicts

- **ITEM-1** — verdict: CONCERN — mirrors `table.tsx`, but must (a) move inter-row spacing into the measured row (gap→padding), (b) tolerate streaming re-measure, (c) keep stable `getItemKey=id` so the measurement cache survives prepend/window-reset. No blocker.
- **ITEM-2** — verdict: PASS — a thin `forwardRef`/`useImperativeHandle` exposing `scrollToMessageId` (id→index→`scrollToIndex`); standard React.
- **ITEM-3** — verdict: CONCERN — jump-to must ensure the target is loaded (via `jumpToMessage`) BEFORE `scrollToIndex`; the initial bottom-jump lands on estimates and relies on follow/re-jump to settle. Contained; guarded by the existing jump + anchor e2e.
- **ITEM-4** — verdict: CONCERN — the #1 risk: index-based prepend anchor must keep the viewport stable while estimated prepended heights settle to measured. Mechanism (`getOffsetForIndex` restore + the virtualizer's `shouldAdjustScrollPositionOnItemSizeChange`) is the standard approach; the scrollTop/scrollHeight anchor e2e is the objective check. Not blocked.
- **ITEM-5** — verdict: PASS — swaps `scrollIntoView` for the `scrollToMessage` callback; loaded→`scrollToIndex`, unloaded→`jumpToMessage` then scroll (same two-branch logic as today).
- **ITEM-6** — verdict: CONCERN — the gallery mock scroll viewport must have a real non-zero height or `useVirtualizer` measures 0 and renders nothing (blank surface + a possible runtime finding). Must verify the gallery cell gives the scroller a height; add a state cell if a new conditional branch appears. No blocker.
