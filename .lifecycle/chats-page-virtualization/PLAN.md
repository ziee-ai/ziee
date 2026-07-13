# PLAN — /chats page conversation-list virtualization

**Feature slug:** `chats-page-virtualization`
**Branch:** `feat/chats-page-virtualization`

## Problem

`/chats` (`ChatHistoryPage` → `ConversationList`) renders **every** loaded
conversation row into the DOM (`visibleConversations.map(...)`, no windowing). As
the user Loads-More across many pages the DOM row count grows unbounded → slow
first paint, janky scroll, heavy re-render on selection. This is a
**scale-performance** defect: a top-level nav feed that does fetch-page + render-ALL.

## Goal

Virtualize the conversation list so only the visible window (+ overscan) of
rows is mounted, scrolling smoothly with **variable** row heights (1- vs 2-line
titles, optional message-count chip) and **no row-height jank** (estimate→measured
correction settles to ~0 after a scroll pause). **Reuse the existing MessageList
virtualizer precedent** — same lib (`@tanstack/react-virtual`), same measured-height
cache + stable-key patterns — do NOT invent a new mechanism.

## JTBD / UX design

A user with hundreds of chats opens `/chats` to **find and reopen** a past
conversation. What they want to DO, per surface:

- **List (primary)** — scroll a long history fluidly; click a card to open it;
  hover to reveal per-row delete; multi-select + bulk-delete. Virtualization must
  be **invisible**: the scrollbar thumb must be stable (no jump as rows measure),
  every card that scrolls into view is fully rendered (no blank rows / pop-in
  flashes), and the "Showing N of M" + **Load More** affordance still bounds the
  set and lets them fetch more. Selection, delete, keyboard focus, and the
  search-empty / error / loading states all keep working unchanged.
- **Search** — typing filters server-side; the (possibly-short) result set renders
  through the same virtualized path with no special-casing.
- **Mobile (~390px, `nativeScroll`)** — the page uses native window-scroll; the
  virtualizer can't observe window scroll, so mobile keeps the **plain** render of
  the (paging-bounded) loaded set — EXACTLY mirroring MessageList's desktop-virtual
  / mobile-plain split. Cards already stack responsively (`ConversationCard`
  `flex-col sm:flex-row`); virtualization doesn't change that.
- **Composability with paging (live4 coordination)** — the list works with
  data that arrives **incrementally** (Load-More appends a page; a new
  conversation optimistically prepends via the `conversation.created` event). The
  virtualizer keys rows by conversation **id** (stable across prepend/append), so
  a growing/prepending set never loses measured heights or teleports. **No
  `ChatHistory.store` change is required** — virtualization is a pure rendering
  layer over the existing `conversations` array + `hasMore`/`loadNextPage`.

### Precedent (UI-surface checklist)

- **Twin surface:** `MessageList.tsx` (the chat message virtualizer) — mirror its
  `useVirtualizer` setup, `estimateSize`, `getItemKey`, `initialMeasurementsCache`
  seed, `measureElement`, coalesced measured-height write-back, DEV metrics, and
  the **desktop-virtualize / mobile-plain** split. The measured-height cache
  (`measuredHeightCache.ts`) is generalized to a small conversation-row analog.
- **Scale/cardinality:** conversation history is **unbounded**. Server paging
  (limit 20/page, `hasMore`, Load-More) bounds the initial + incremental load;
  virtualization bounds the **DOM** to the visible window regardless of how many
  pages are loaded. The "Showing N of M" affordance already exists — kept.
- **Device/responsive:** desktop = inner OverlayScrollbars viewport, virtualized;
  mobile (`nativeScroll`) = plain window-scroll render. Gallery coverage adds a
  populated + a narrow (390px) state.
- **Populated-render review:** a new gallery surface seeds ~200 conversations so
  the design-critic + runtime passes review the REAL virtualized, data-populated
  list (not just the existing loading arm).
- **User-visible progress:** Load-More button keeps its `loadingMore` spinner;
  the aria-live "Showing N of M" status stays.
- **Input economy / multi-instance / platform affordances:** N/A (no new inputs,
  single surface, no new chrome).

## Items

- **ITEM-1**: Content-aware first-pass **row-height estimator** for a conversation
  card (`estimateConversationHeight(conv, width)`): base card chrome + 1-vs-2-line
  title (title length ÷ chars-per-line at width) + optional message-count/meta row.
  Cheap, total (undefined → floor), memoized per (conversation, width bucket).
  Mirrors `estimateMessageHeight.ts`. Cuts the estimate→measured correction (the
  scrollbar-jump / jank signal) toward zero.
- **ITEM-2**: **Measured-height cache** for conversation rows — persist real
  measured heights across mounts, keyed by conversation id + width bucket, seeded
  into `useVirtualizer({ initialMeasurementsCache })` so re-opening `/chats` starts
  rows at their true height (near-zero first-scroll correction). Generalize the
  existing `measuredHeightCache.ts` into a **shared, reusable** module both
  MessageList and the new list consume (no duplicated LRU), OR a thin
  conversation-scoped analog if generalization risks the message path. (DEC-2.)
- **ITEM-3**: **Virtualized row renderer** — a new
  `VirtualizedConversationList` component that takes the ordered `conversations`,
  the resolved scroll element, and the per-row render props, and mounts only the
  visible window via `useVirtualizer` (`estimateSize` from ITEM-1,
  `getItemKey: id`, `measureElement`, `overscan`, seed from ITEM-2). Absolutely-
  positioned rows inside a `height=getTotalSize()` container; **inter-row spacing
  lives INSIDE the measured row** (a flex gap is lost under absolute positioning —
  the MessageList DEC-6 lesson). Renders each `ConversationCard` unchanged.
- **ITEM-4**: **Scroll-element wiring** in `ConversationList` — attach a ref to the
  card-list `DivScrollY`, flip a `scrollerReady` state on its OverlayScrollbars
  `initialized` event, resolve the viewport root via `osInstance().elements()
  .viewport`, and pass it as `getScrollElement`. Mirrors `ConversationPage`
  lines 100-127 + 592-622. **Desktop virtualizes; mobile (`nativeScroll`, no OS
  instance → null root) renders plain.** Set the virtualizer `scrollMargin` to the
  virtual container's offset within the scroller so the transient bulk-actions bar
  / padding above the rows doesn't skew item offsets. Keep the "Showing N of M" +
  Load-More footer as a **non-virtualized sibling below** the virtual container.
- **ITEM-5**: **Surgical ConversationList integration** — replace ONLY the inner
  card `.map()` block (current lines ~206-249) with the virtualized renderer
  (desktop) / plain map (mobile); leave the search-box portal, bulk-actions bar,
  empty/error/loading arms, and store wiring byte-untouched. Keep the change a
  localized swap so it merges cleanly with live4's paging edits to the same file.
- **ITEM-6**: **DEV-only jank metrics** — expose `window.__CHATLIST_METRICS__`
  (`corrections` counter incremented on non-sync size recorrections + `reset()` +
  `totalSize()`), compiled out of production via `import.meta.env.DEV`. Mirrors
  MessageList's `__MSGLIST_METRICS__`. This is what the behavioural e2e asserts
  settles to ~0 after a scroll pause (RUNS the no-jank claim — B7).
- **ITEM-7**: **Gallery surface** `seeded-conversation-list-long` — a backend-free
  surface seeding ~200 conversations into `ChatHistory.store` driving the REAL
  `ConversationList` inside a fixed-height scroll box, plus a **narrow (390px)**
  variant, for the design-critic/runtime passes AND the behavioural window/scroll
  e2e. Mirrors `MessageListLongDemo.tsx` + its gallery entry. Add the required
  state-matrix / gallery-coverage registration so `npm run check` passes.

## Files to touch

New:
- `src-app/ui/src/modules/chat/core/utils/estimateConversationHeight.ts` (ITEM-1)
- `src-app/ui/src/modules/chat/core/utils/estimateConversationHeight.test.ts` (ITEM-1 unit)
- `src-app/ui/src/modules/chat/components/VirtualizedConversationList.tsx` (ITEM-3/4/6)
- `src-app/ui/src/dev/gallery/ConversationListLongDemo.tsx` (ITEM-7)
- `src-app/ui/tests/e2e/chat/conversation-list-virtualization.spec.ts` (behavioural e2e)

Edit (surgical):
- `src-app/ui/src/modules/chat/components/ConversationList.tsx` (ITEM-5 — swap the
  inner card map + scroller ref wiring; keep everything else)
- `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.ts` (ITEM-2 — generalize
  keys OR add a sibling; decided in DECISIONS)
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx` + gallery coverage/state registration
  files as required by `check:gallery-coverage` / `check:state-matrix` (ITEM-7)

Possibly-generated (verify, don't hand-edit): none — no OpenAPI/type change.

## Patterns to follow

- **Virtualization / measured-height / metrics** → `MessageList.tsx` +
  `measuredHeightCache.ts` + `estimateMessageHeight.ts` (the exact precedent named
  in the brief). Same `@tanstack/react-virtual` `useVirtualizer` shape.
- **Scroll-element resolution (OverlayScrollbars viewport + `scrollerReady`)** →
  `ConversationPage.tsx` (`getViewport`, `events={{ initialized }}`,
  `getScrollElement={() => getViewport()?.root ?? null}`).
- **Gallery long-list demo + DEV metrics e2e** → `MessageListLongDemo.tsx` +
  `tests/e2e/visual/chat-scroll-stability.spec.ts` (`__MSGLIST_METRICS__`,
  `g-msglist-scroll` scroller testid).
- **Real seeded /chats e2e** → `tests/e2e/chat/conversation-list-load-more.spec.ts`
  (seed N conversations via `POST /api/conversations`, `goto('/chats')`).
- **Store usage / no store change** → `ChatHistory.store.ts` already exposes
  `conversations`, `hasMore`, `total`, `loadNextPage`; the virtualizer is a pure
  render layer (keeps live4 merge surface minimal).
