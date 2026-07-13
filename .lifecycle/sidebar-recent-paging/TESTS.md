# TESTS — enumerated coverage (every ITEM ↔ ≥1 TEST)

Tiers: `unit` = vitest store test; `e2e` = Playwright. No backend change → no
`integration` (Rust) tier. UI diff → ≥1 `tier: e2e` REQUIRED (present: TEST-6..9).

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `loadRecentConversations(1)` (mock `ApiClient.Conversation.list` returning 20 of total 45) populates `recentConversations` (len 20), sets `recentTotal=45`, `recentHasMore=true`, `recentInitialized=true`, `recentPage=1`, and calls the endpoint with `sort:'recent'` + NO `search`.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: after page-1 load, `loadMoreRecent()` fetches page 2 and APPENDS (len 40), advances `recentPage=2`, keeps `recentHasMore=true`; a duplicate id across the page boundary is de-duplicated (no double row); a call while `recentLoadingMore` is a no-op.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: paging to the final page sets `recentHasMore=false` (loaded === total), and a subsequent `loadMoreRecent()` is a no-op (no extra fetch).
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: `loadConversations(1)` (unfiltered, recent) populates `conversations`/`total`/`hasMore` but does NOT mutate `recentConversations` (decoupled) — pre-seeded `recentConversations` survives a history-page reload unchanged.
- **TEST-5** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/chat/stores/ChatHistory.store.test.ts` — asserts: a `conversation.created` event prepends to `recentConversations` WITHOUT truncating a >20 loaded list (len 40 → 41, new id at index 0) and bumps `recentTotal`; a `sync:conversation`/`deleteConversation` delete of a loaded row removes it and decrements `recentTotal`.
- **TEST-6** (tier: e2e) [covers: ITEM-2, ITEM-6] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: seed 45 conversations, land on `/chats`, the sidebar `chat-recent-conversations-menu` renders the FIRST page (~20 rows, and NOT all 45) on initial load.
- **TEST-7** (tier: e2e) [covers: ITEM-3, ITEM-6, ITEM-7] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: scrolling the sidebar list to the bottom auto-loads the NEXT page (row count grows past the first page toward 45) — the load happens on scroll WITHOUT any manual button click; the `chat-recent-loading-more` indicator appears during the fetch.
- **TEST-8** (tier: e2e) [covers: ITEM-3, ITEM-7] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: after scrolling through all pages the list reaches all 45 rows and STOPS — no further growth, no loading indicator, no sentinel-triggered fetch on further scroll (end-of-list).
- **TEST-9** (tier: e2e) [covers: ITEM-5, ITEM-6] file: `src-app/ui/tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` — asserts: with the sidebar scrolled past page 1, creating a new conversation (New chat) makes the new chat appear at the TOP of the sidebar list while the previously-loaded older rows remain present (not reset to 20).
- **TEST-10** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/visual/*` (gallery, via `gate:ui`/`runtime-health`) — asserts: the new seeded widget surfaces "Recent chats — loaded (many, has more)" and "Recent chats — loading more" render with zero runtime HIGH findings (no console error/exception/failed request/AA-contrast) at desktop and narrow (390px) viewport. (Run as part of `npm run gate:ui` in phase 8; it is the browser-verify harness per A6/A7.)

## Coverage map (every ITEM covered)

- ITEM-1 → TEST-1
- ITEM-2 → TEST-1, TEST-6
- ITEM-3 → TEST-2, TEST-3, TEST-7, TEST-8
- ITEM-4 → TEST-4
- ITEM-5 → TEST-5, TEST-9
- ITEM-6 → TEST-6, TEST-7, TEST-9
- ITEM-7 → TEST-7, TEST-8
- ITEM-8 → TEST-10

## Notes

- No new permission introduced → no `[negative-perm]` restricted-user e2e
  required (A10 N/A). The widget reuses `ConversationsRead`, already gated on the
  store fetch + the sidebar slot; the existing suite covers that gate.
- No cosmetic tests: the unit tests drive the real store actions against a mocked
  `ApiClient` boundary (external boundary only, per [[feedback_no_cosmetic_tests]]);
  the e2e drives the REAL sidebar in a real browser with real seeded data and a
  real scroll (B7 — RUNS the behavior, not reads it).
