# PLAN — Sidebar Recent-Chats infinite-scroll paging

## Problem / goal

The left-sidebar "Recent chats" widget
(`RecentConversationsWidget` → `Stores.ChatHistory.recentConversations`) today
renders a fixed `slice(0, 20)` of the user's most-recent conversations with **no
incremental paging** — a user with >20 conversations can never reach the 21st
from the sidebar. GOAL: **infinite-scroll paging** — load the first page, then
auto-load the next page as the user scrolls near the bottom of the sidebar list,
show a loading indicator while fetching, and stop cleanly when there are no more.

This is a **nav-feed** surface, so infinite-scroll / load-on-scroll is the
correct idiom (NOT the numbered `ListPagination` used on settings/detail pages;
NOT the manual "Load More" button used on the `/chats` history page — that page
is a management view, the sidebar is a feed).

## Key survey findings (drive the whole plan)

1. **Backend already fully supports offset/limit paging** — no backend change.
   `GET /api/conversations` (`chat/core/handlers/conversations.rs::list_conversations`)
   takes `page`/`limit`(≤100)/`search`/`sort`, computes `offset=(page-1)*limit`,
   and returns `ConversationListResponse { conversations, total }`. The store
   already calls it with `{ page, limit, search, sort }`.
2. **The store already has a full paging engine** for the `/chats` history list:
   `page`/`limit`/`total`/`hasMore`/`loadingMore` + `loadNextPage()` +
   `loadConversations(page)`. The `/chats` `ConversationList.tsx` consumes it via
   a `data-testid="chat-history-load-more-btn"` Load-More button. This is the
   precedent to MIRROR for the store shape.
3. **`recentConversations` is DELIBERATELY a separate list** from the
   search/sort-mutable `conversations`: it must always be the "true most-recent"
   set (unfiltered, `recent` sort). It is currently populated as a *side-effect*
   of `loadConversations(1)` (`recentConversations = pageItems.slice(0,20)`, only
   on an unfiltered default-sort page-1 load) and capped at 20 by the
   `conversation.created` handler's `.slice(0,20)`.
   → For independent infinite scroll the sidebar needs its **own** paging state,
   decoupled from the history query (otherwise a history-page unfiltered page-1
   reload would silently reset the accumulated sidebar back to 20 and jump the
   scroll — a real bug). See DEC-1.
4. **Virtualization precedent exists**: `kit/table.tsx::VirtualTable`
   (`useVirtualizer` from `@tanstack/react-virtual`, already a dep) virtualizes
   rows INSIDE an OverlayScrollbars viewport — `getScrollElement: () =>
   osRef.current?.osInstance()?.elements().viewport ?? null`,
   `events={{ initialized: () => setScrollReady(true) }}` to re-render once the OS
   viewport exists (the `defer` race), fixed `estimateSize`, `overscan`. This is
   the primary precedent to MIRROR (`MessageList.tsx` + `kit/multi-select.tsx` are
   secondary). The infinite-scroll TRIGGER with a virtualizer is idiomatic: watch
   the LAST virtual item and fetch when it nears the end (no IntersectionObserver
   sentinel needed).
5. **The kit `<Menu items>` materializes ALL rows** (`menu.tsx::Items` maps every
   item) — incompatible with virtualization. So the virtualized sidebar list must
   render its own windowed rows, REUSING the Menu row's visual styling (shared
   class/helper extracted from `menu.tsx`, not re-derived) so it stays pixel-
   faithful to the nav/tools menus above it. Row SEMANTICS become a list of
   navigation buttons (`role="list"`), not a virtualized `role="menu"` — a
   virtualized menu can't honor the ARIA menu keyboard contract across
   non-rendered items (see DEC-8).
6. **Desktop needs no separate edit**: `src-app/desktop/ui` has no override of
   `RecentConversationsWidget.tsx`/`ChatHistory.store.ts`; its `localOverridePlugin`
   falls back to core `../../ui/src`, so the core change applies to desktop
   automatically. The diff will NOT touch `src-app/desktop/ui/**` (no R2-3 desktop
   override to diff).
7. **No new permission, no new migration.** The widget reuses the existing
   `ConversationsRead` gate (already on the store fetch + the slot). So A9/A10
   (permission deny tests) do NOT apply.

## Items

- **ITEM-1**: Add dedicated recent-list paging state to `ChatHistory.store.ts`,
  mirroring the existing `conversations` paging fields but namespaced `recent*`:
  `recentPage` (default 1), `recentTotal` (0), `recentHasMore` (false),
  `recentLoading` (false), `recentLoadingMore` (false), `recentInitialized`
  (false). (Reuse the existing `limit` field as the shared page size — DEC-4.)
- **ITEM-2**: Add `loadRecentConversations(page?)` action — fetches the
  conversations list endpoint UNFILTERED + `sort: 'recent'` at the target page
  (default `recentPage`); on page 1 REPLACES `recentConversations`, on later
  pages this action is not used (page-1 loader only). Sets
  `recentTotal`/`recentHasMore` (= loaded < total) / `recentInitialized`, and
  `recentLoading` (page 1) in-flight. Dedupes concurrent calls via the
  `recentLoading`/`recentLoadingMore` in-flight guard (mirrors `loadConversations`).
  Permission-gated by `hasPermissionNow(ConversationsRead)` like the existing fetch.
- **ITEM-3**: Add `loadMoreRecent()` action — guards `!recentHasMore ||
  recentLoadingMore`, fetches page `recentPage + 1` (unfiltered + `recent`),
  APPENDS to `recentConversations` deduped by `id`, updates
  `recentPage`/`recentHasMore`/`recentLoadingMore`. Mirrors the existing
  `loadNextPage()`.
- **ITEM-4**: Decouple the history query from the sidebar list — remove the
  `recentConversations = pageItems.slice(0,20)` side-effect from
  `loadConversations`. `loadConversations` keeps owning `conversations`/`page`/
  `hasMore`/`total` for `/chats`; the sidebar now owns `recentConversations`
  entirely via ITEM-2/3.
- **ITEM-5**: Update the store event handlers to keep the PAGED recent list
  consistent WITHOUT the old 20-cap:
  `conversation.created` prepends (dedup by id, **no** `.slice(0,20)`) and bumps
  `recentTotal`; `conversation.titleUpdated` unchanged (title map already covers
  `recentConversations`); `sync:conversation` delete decrements `recentTotal`
  when the row was in `recentConversations`; `sync:conversation` CREATE/UPDATE
  merge-prepends the new page-1 rows via a dedicated `syncRecentFront()` action
  (fetch page 1, prepend only the not-yet-seen ids, recompute
  `recentTotal`/`recentHasMore`) — a page-1 REPLACE would collapse an
  infinite-scrolled sidebar back to one page and jump the scroll, so the sync
  path must PRESERVE the accumulated pages (amended from the original "reload
  page 1"; see DRIFT-1); `sync:reconnect` still does a full page-1 replace
  (`loadRecentConversations(1)` — a fresh view after an offline gap is correct);
  `deleteConversation` + `bulkDelete` decrement `recentTotal` by the count
  actually removed from `recentConversations`.
- **ITEM-6**: `RecentConversationsWidget.tsx` — VIRTUALIZED infinite scroll
  (mirror `kit/table.tsx::VirtualTable`):
  - switch the mount effect to `if (!recentInitialized)
    Stores.ChatHistory.loadRecentConversations()`, and gate loading/empty on
    `recentLoading`/`recentInitialized` (was `loading`/`isInitialized`);
  - render the "Recent chats" caption as a standalone header ABOVE the scroll
    area (not inside the virtual list);
  - capture the `DivScrollY` OverlayScrollbars ref; pass
    `events={{ initialized: () => setScrollReady(true) }}` so the virtualizer
    re-measures once the viewport exists; `getScrollElement = () =>
    osRef.current?.osInstance()?.elements().viewport ?? null`;
  - `useVirtualizer({ count: recentConversations.length, getScrollElement,
    estimateSize: () => ROW_H, overscan: 8 })`; render ONLY
    `virt.getVirtualItems()` as absolutely-positioned rows inside a
    `height: virt.getTotalSize()` spacer;
  - **auto-load trigger**: an effect watching the last virtual item — when
    `lastItem.index >= recentConversations.length - 1 && recentHasMore &&
    !recentLoadingMore` call `Stores.ChatHistory.loadMoreRecent()` (the
    tanstack-virtual infinite-scroll idiom; replaces an IntersectionObserver
    sentinel).
- **ITEM-7**: Loading-more + end affordance — while `recentLoadingMore`, render a
  centered `<Spin label="Loading more" />` row (`aria-live="polite"`/`role="status"`)
  pinned just below the virtualized rows (offset by `getTotalSize()`); when
  `!recentHasMore` the list simply ends (no indicator, no chrome — the `/chats`
  page keeps the numeric "Showing N of M", the feed does not — DEC-5).
- **ITEM-9**: Virtual row rendering, faithful to the kit Menu row — reuse the
  Menu row's visual styling by extracting a shared class/helper from `menu.tsx`
  (one source of truth; NO re-derivation), used by both `menu.tsx::Items` and the
  sidebar virtual row. Each row preserves: truncated title, the hover-reveal
  `ConversationRowActions` kebab (the existing `group/menu-row` reveal), and the
  selected treatment via `aria-current="page"` for the open conversation. Row
  semantics = `role="list"` + navigation `<button>` rows (DEC-8), keyboard = Tab +
  click; per-row `aria-setsize={recentTotal}`/`aria-posinset={index+1}` so the
  virtualized window still exposes list position to assistive tech.
- **ITEM-8**: Gallery coverage for the widget's NEW conditional render states so
  `check:state-matrix` (inside `npm run check`) stays green: add seeded gallery
  surfaces "Recent chats — loaded (many, has more)" and "Recent chats — loading
  more", mirroring the existing `seeded-recent-convos-loading`/`-empty` entries;
  regenerate the gallery coverage/state-matrix generated files.

## Files to touch

- `src-app/ui/src/modules/chat/stores/ChatHistory.store.ts` — ITEM-1..5.
- `src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx` — ITEM-6,7,9.
- `src-app/ui/src/components/ui/kit/menu.tsx` — ITEM-9 (extract + export the row
  visual-style class/helper; `Items` uses it — output-identical refactor so the
  nav/tools menus are unchanged).
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx` — ITEM-8 (new seeded states,
  seeded with ENOUGH rows — ~40 — that windowing is observable).
- `src-app/ui/src/dev/gallery/*.generated.ts` + coverage/state files — ITEM-8
  (regenerated via the gallery gen script, not hand-edited).
- `src-app/ui/src/components/ui/testIds.generated.ts` — regenerated if a new kit
  testid literal is introduced (sentinel/loading-row testids are on plain divs;
  regen defensively).
- `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — NEW unit test
  (paging reducer logic).
- `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — NEW e2e.

## Patterns to follow

- **Store paging** → mirror the SAME FILE's existing `loadConversations` /
  `loadNextPage` / `hasMore` / `loadingMore` / in-flight-guard / dedup idioms.
  The `recent*` fields and actions are a namespaced parallel of them.
- **Virtualization inside an OverlayScrollbars viewport** → mirror
  `src/components/ui/kit/table.tsx::VirtualTable` (`useVirtualizer` +
  `getScrollElement` from `osInstance().elements().viewport` +
  `events={{ initialized }}` ready-trigger + fixed `estimateSize` + `overscan`).
  Infinite-scroll trigger = watch the last virtual item (tanstack idiom).
  Secondary refs: `src/modules/chat/components/MessageList.tsx`,
  `src/components/ui/kit/multi-select.tsx`.
- **Widget structure** → the sidebar widget is its OWN precedent (a unique
  surface — no sibling to twin). Keep its `DivScrollY` scroll container and reuse
  the existing `<Spin label="Loading" />` for the loading-more row. The virtual
  rows reuse the kit Menu ROW styling (ITEM-9) so they stay pixel-faithful to the
  Navigation/Tools menus above them.
- **Gallery seeding** → mirror the existing `seeded-recent-convos-loading` /
  `seeded-recent-convos-empty` entries in `seededSurfaces.tsx`.
- **Store unit test** → mirror `src/modules/voice/stores/VoiceModel.store.test.ts`
  (mock `ApiClient`, drive actions, assert state).
- **e2e** → mirror `tests/e2e/chat/conversation-list-search.spec.ts` (POST-loop
  seeding to `/api/conversations` with the admin token) + the sidebar targeting
  in `tests/e2e/projects/sidebar-menu.spec.ts` (`chat-recent-*` testids, land on
  `/chats` so the sidebar is visible).

## UI-surface checklist (the one surface this feature changes: the sidebar
## "Recent chats" widget)

- **Precedent** — the widget is a unique sidebar feed with no exact sibling; the
  closest paging precedents are (a) the `/chats` `ConversationList` for the STORE
  paging shape, and (b) `RawCodeView`/`ConversationPage` for the scroll-sentinel.
  Mirror those mechanisms; keep the widget's existing `Menu`/`DivScrollY` visual
  structure unchanged. The IDIOM choice is deliberate: feed → infinite-scroll,
  not the settings `ListPagination` and not the history-page manual button.
- **Scale / cardinality** — MAX size = the user's total conversation count
  (unbounded; a heavy user could have thousands). Initial load is bounded to ONE
  page (`limit: 20`); server paging fetches 20-row pages on scroll. The rendered
  DOM is bounded by **row virtualization** (`@tanstack/react-virtual`, mirror
  `kit/table.tsx`): only the visible window + overscan is in the DOM regardless of
  how many pages have loaded, so scrolling a thousand-conversation history stays
  O(viewport). This is the two-layer bound the checklist requires: bounded
  network (paging) AND bounded DOM (virtualization).
- **Device size / responsive** — the widget lives in the sidebar, which the
  app-layout already hides/collapses on mobile (the sidebar is a drawer/off-canvas
  at ~390px). Within the sidebar the list is `min-h-0 flex-1` inside `DivScrollY`,
  so it scrolls independently at every width; the infinite-scroll behavior is
  width-independent (viewport-root observer). No new breakpoint logic. Gallery
  coverage will include the narrow-viewport render (per gate:ui) via the seeded
  widget surface.
- **User-visible progress** — while a next page is fetching, a centered `Spin`
  row ("Loading more") appears at the list bottom, announced via
  `aria-live="polite"`. Initial page uses the existing full-widget spinner. When
  exhausted (`!recentHasMore`) the list ends with no spinner — the user SEES the
  list stop growing, which is the expected infinite-scroll signal.
- **Input economy** — N/A (no user input on this surface; it is a click-to-navigate
  list). No values to auto-detect.
- **JTBD design** — see the JTBD section below.

## JTBD (jobs-to-be-done) — sidebar Recent chats

A real user's job here is **"find and reopen one of my past conversations from
the sidebar without leaving my current context."** Enumerated across the
surfaces this feature exposes:

- **List (loaded)** — I glance at the sidebar, see my most-recent chats, and
  click one to reopen it. Today I can see at most 20; if the chat I want is the
  25th most-recent, I'm stuck and must detour to the `/chats` page. JOB: let me
  keep scrolling the sidebar and have older chats appear seamlessly until I find
  it. → infinite scroll (ITEM-2/3/6).
- **Loading (initial)** — I just logged in / opened the app: I want to know the
  list is coming, not that it's empty. → existing full-widget `Spin` (kept).
- **Loading more (scroll)** — I scrolled to the bottom and there ARE more: I want
  a clear "fetching more" signal so I don't think the list just ended. → the
  `Spin` "Loading more" row (ITEM-7).
- **End of list** — I've scrolled to my oldest chat: the list should simply stop,
  with no spinner spinning forever and no "load more" button teasing a next page
  that doesn't exist. → `!recentHasMore` → no sentinel, no chrome (ITEM-3/7).
- **Empty** — brand-new account, zero chats: I see the friendly "No conversations
  yet" empty state, not a spinner. → existing empty state (kept, re-gated on
  `recentInitialized`).
- **Live updates while scrolled** — I start a new chat (or one syncs from another
  device) while the sidebar is scrolled deep: the newest chat must appear at the
  TOP and my already-loaded older pages must NOT be dropped or reset to 20. →
  `conversation.created`/`sync` handlers prepend + dedup, keep loaded pages,
  bump `recentTotal` (ITEM-5). This is exactly what the old `.slice(0,20)` broke.
- **Delete** — I delete a chat from the row menu: it disappears from the sidebar
  and the count bookkeeping stays consistent so `recentHasMore` doesn't desync. →
  `deleteConversation`/`bulkDelete`/sync-delete decrement `recentTotal` (ITEM-5).
- **Mobile** — the sidebar is an off-canvas drawer; the same list scrolls and
  pages inside it. No desktop-only behavior.
