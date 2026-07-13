# TESTS — enumerated coverage (every ITEM ↔ ≥1 TEST)

Tiers: `unit` = vitest store test; `e2e` = Playwright. No backend change → no
`integration` (Rust) tier. UI diff → ≥1 `tier: e2e` REQUIRED (present: TEST-6..9).

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `loadRecentConversations(1)` (mock `ApiClient.Conversation.list` returning 20 of total 45) populates `recentConversations` (len 20), sets `recentTotal=45`, `recentHasMore=true`, `recentInitialized=true`, `recentPage=1`, and calls the endpoint with `sort:'recent'` + NO `search`.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: after page-1 load, `loadMoreRecent()` fetches page 2 and APPENDS (len 40), advances `recentPage=2`, keeps `recentHasMore=true`; a duplicate id across the page boundary is de-duplicated (no double row); a call while `recentLoadingMore` is a no-op.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: paging to the final page sets `recentHasMore=false` (loaded === total), and a subsequent `loadMoreRecent()` is a no-op (no extra fetch).
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `loadConversations(1)` (unfiltered, recent) populates `conversations`/`total`/`hasMore` but does NOT mutate `recentConversations` (decoupled) — pre-seeded `recentConversations` survives a history-page reload unchanged.
- **TEST-5** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: a `conversation.created` event prepends to `recentConversations` WITHOUT truncating a >20 loaded list (len 40 → 41, new id at index 0) and bumps `recentTotal`; a `sync:conversation`/`deleteConversation` delete of a loaded row removes it and decrements `recentTotal`.
- **TEST-3b** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: a SHORT server page (fewer than `limit`) sets `recentHasMore=false` even when `recentTotal` is drifted high, and a further `loadMoreRecent()` does NOT fetch — the fix for the runaway-empty-page-fetch HIGH finding.
- **TEST-3c** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: a next page that appends nothing new (pure dedup overlap) sets `recentHasMore=false` (no-progress guard), so a stalled cursor can't loop.
- **TEST-3d** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: a first-load failure sets `recentError` (not a stuck spinner) and a retry after recovery clears it and loads — the fix for the stuck-spinner regression.
- **TEST-5b** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `syncRecentFront()` (the sync-CREATE path) fetches page 1 and MERGE-prepends only the not-yet-seen rows to the front, PRESERVING the accumulated older pages (a 40-row loaded list gains the new top row → 41, older rows still present) — i.e. it does NOT reset the sidebar to one page.
- **TEST-6** (tier: e2e) [covers: ITEM-2, ITEM-6] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: seed 45 conversations, land on `/chats`, the sidebar recent list (`chat-recent-conversations-list`) shows the newest conversation on initial load, the oldest is absent (only page 1 loaded, ≤20 rows in DOM), and the list exposes `role="list"` with `aria-setsize`/`aria-posinset` on its rows.
- **TEST-7** (tier: e2e) [covers: ITEM-3, ITEM-6, ITEM-7] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: scrolling the sidebar list downward reveals OLDER conversations not initially present (a title from page 2/3 becomes visible) — loading happens on scroll WITHOUT any manual button click; the `chat-recent-loading-more` indicator appears during a fetch.
- **TEST-8** (tier: e2e) [covers: ITEM-3, ITEM-7] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: scrolling to the very bottom reaches the OLDEST seeded conversation (all pages loaded) and STOPS — the loading indicator is gone and further scroll triggers no more fetches (end-of-list, `recentHasMore` false).
- **TEST-9** (tier: e2e) [covers: ITEM-5, ITEM-6] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: after scrolling past page 1, creating a new conversation makes it appear at the TOP of the sidebar list while a previously-loaded older row still resolves (the accumulated list is not reset to 20).
- **TEST-5c** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `syncRecentFront()` re-anchors `recentPage` to the grown length after a full-page cross-device prepend, so the next `loadMoreRecent()` fetches the correct older page and paging keeps progressing (does not dead-end on an all-overlap page).
- **TEST-13** (tier: e2e) [covers: ITEM-3, ITEM-6] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: with every next-page request forced to 500, repeated scroll-to-bottom does NOT hammer the API — the number of failed page fetches stays bounded (≤5) and the list keeps showing page 1 (the `!recentError` auto-load failure gate).
- **TEST-10** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: a virtual row is faithful to the Menu row — the currently-open conversation row carries `aria-current="page"`, and hovering a row reveals its `chat-recent-row-actions-btn-<id>` kebab whose Delete removes the row (row-actions still work under virtualization).
- **TEST-11** (tier: e2e) [covers: ITEM-6, ITEM-9] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: VIRTUALIZATION windowing — after scrolling all 45 rows in so the oldest is visible, the NEWEST (top) row is UNMOUNTED (count 0) and the DOM never holds all 45 at once. A non-virtualized list would keep the top row mounted; this off-screen-unmount is the decisive proof.
- **TEST-12** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/visual/*` (gallery, via `gate:ui`/`runtime-health`) — asserts: the new seeded widget surfaces "Recent chats — loaded (many, has more)" and "Recent chats — loading more" render with zero runtime HIGH findings (no console error/exception/failed request/AA-contrast) at desktop and narrow (390px) viewport. (Run as part of `npm run gate:ui` in phase 8; it is the browser-verify harness per A6/A7.)

## Coverage map (every ITEM covered)

- ITEM-1 → TEST-1
- ITEM-2 → TEST-1, TEST-3b, TEST-3d, TEST-6
- ITEM-3 → TEST-2, TEST-3, TEST-3b, TEST-3c, TEST-7, TEST-8, TEST-13
- ITEM-4 → TEST-4
- ITEM-5 → TEST-5, TEST-5b, TEST-5c, TEST-9
- ITEM-6 → TEST-6, TEST-7, TEST-9, TEST-11, TEST-13
- ITEM-7 → TEST-7, TEST-8
- ITEM-8 → TEST-12
- ITEM-9 → TEST-10, TEST-11

## Notes

- No new permission introduced → no `[negative-perm]` restricted-user e2e
  required (A10 N/A). The widget reuses `ConversationsRead`, already gated on the
  store fetch + the sidebar slot; the existing suite covers that gate.
- No cosmetic tests: the unit tests drive the real store actions against a mocked
  `ApiClient` boundary (external boundary only, per [[feedback_no_cosmetic_tests]]);
  the e2e drives the REAL sidebar in a real browser with real seeded data and a
  real scroll (B7 — RUNS the behavior, not reads it).
