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
4. **Scroll-sentinel precedent exists**: `RawCodeView.tsx:295-333` (and
   `pdf/body.tsx`, `kit/table.tsx`) get the OverlayScrollbars viewport via
   `osRef.current?.osInstance()?.elements().viewport`, RAF-retry until it exists
   (OverlayScrollbars `defer`), and use it as the `IntersectionObserver` `root`
   with a `rootMargin` prefetch band. `ConversationPage.tsx:147-160` is the
   bottom-sentinel form. MIRROR these for the sidebar.
5. **Desktop needs no separate edit**: `src-app/desktop/ui` has no override of
   `RecentConversationsWidget.tsx`/`ChatHistory.store.ts`; its `localOverridePlugin`
   falls back to core `../../ui/src`, so the core change applies to desktop
   automatically. The diff will NOT touch `src-app/desktop/ui/**` (no R2-3 desktop
   override to diff).
6. **No new permission, no new migration.** The widget reuses the existing
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
  when the row was in `recentConversations`; `sync:reconnect` reloads recent
  page 1 (`loadRecentConversations(1)`); `deleteConversation` + `bulkDelete`
  decrement `recentTotal` by the count actually removed from `recentConversations`.
- **ITEM-6**: `RecentConversationsWidget.tsx` — wire infinite scroll:
  - switch the mount effect to `if (!recentInitialized)
    Stores.ChatHistory.loadRecentConversations()`, and gate loading/empty on
    `recentLoading`/`recentInitialized` (was `loading`/`isInitialized`);
  - capture the `DivScrollY` ref, resolve its OverlayScrollbars viewport
    (`osInstance().elements().viewport`, RAF-retry per RawCodeView), and attach an
    `IntersectionObserver(root=viewport, rootMargin:'200px 0px')` on a bottom
    **sentinel** `<div>` rendered after the `Menu`; on intersect call
    `Stores.ChatHistory.loadMoreRecent()`. Re-attach when `recentConversations`
    first becomes non-empty (sentinel only mounts once rows exist) and disconnect
    on unmount.
- **ITEM-7**: Loading-more + end affordance in the widget — while
  `recentLoadingMore`, render a centered `<Spin label="Loading more" />` row
  (with `aria-live="polite"`/`role="status"` so it's announced) below the Menu;
  when `!recentHasMore` the list simply ends (no sentinel, no chrome — standard
  infinite-scroll idiom; the `/chats` page keeps the numeric "Showing N of M",
  the feed does not — DEC-5).
- **ITEM-8**: Gallery coverage for the widget's NEW conditional render states so
  `check:state-matrix` (inside `npm run check`) stays green: add seeded gallery
  surfaces "Recent chats — loaded (many, has more)" and "Recent chats — loading
  more", mirroring the existing `seeded-recent-convos-loading`/`-empty` entries;
  regenerate the gallery coverage/state-matrix generated files.

## Files to touch

- `src-app/ui/src/modules/chat/stores/ChatHistory.store.ts` — ITEM-1..5.
- `src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx` — ITEM-6,7.
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx` — ITEM-8 (new seeded states).
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
- **Scroll sentinel / OverlayScrollbars root** → mirror
  `src/modules/file/viewers/shared/RawCodeView.tsx:295-333`
  (`osInstance().elements().viewport` + RAF-retry + `IntersectionObserver` with a
  `rootMargin` band) and the bottom-sentinel form in
  `src/modules/chat/pages/ConversationPage.tsx:147-160`.
- **Widget structure** → the sidebar widget is its OWN precedent (a unique
  surface — no sibling to twin). Keep its `Menu` + `DivScrollY` structure and
  reuse the existing `<Spin label="Loading" />` for the loading-more row.
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
  page (`limit: 20`). Growth is by 20-row pages on scroll. No virtualization —
  mirrors the `/chats` `ConversationList` (which also renders accumulated rows
  un-virtualized); acceptable because rows are lightweight `Menu` items and the
  sidebar viewport shows a small window. (If a future heavy-user perf issue
  appears, virtualization is the follow-up — recorded as a candidate, not built
  now, to match precedent — see the audit's scale-performance angle.)
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
