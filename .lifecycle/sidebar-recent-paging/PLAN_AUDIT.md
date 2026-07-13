# PLAN_AUDIT — plan vs codebase

## Breakage risk

- **`loadConversations` decoupling (ITEM-4)** — the ONLY reader of
  `recentConversations` is `RecentConversationsWidget`. Removing the
  `recentConversations = pageItems.slice(0,20)` side-effect from
  `loadConversations` therefore cannot break the `/chats` history page (which
  reads `conversations`, not `recentConversations`). Verified consumers of
  `loadConversations`: `ConversationList.tsx` (mount effect), `ChatHistoryPage.tsx`
  (refetch), and the sidebar (switched to `loadRecentConversations` by ITEM-6).
  After the switch, the sidebar self-initializes; the history page is unaffected.
  Risk: LOW. The old comment in `ConversationList.tsx:61` ("the sidebar calls
  loadConversations() at login and flips isInitialized") becomes stale — but
  `ConversationList`'s effect already calls `loadConversations()` unconditionally,
  so its own `isInitialized` is still set. No functional break.
- **Event handlers (ITEM-5)** — `conversation.created`/`titleUpdated`/`sync:*`
  already mutate `recentConversations`; the edits change the truncation/total
  bookkeeping only, preserving the existing filter/dedup guards. No new event
  keys. Risk: LOW.
- **IntersectionObserver on OS viewport (ITEM-6)** — the exact pattern is proven
  in `RawCodeView.tsx`; the OverlayScrollbars `defer` null-viewport race is
  handled by the RAF retry. In the dev gallery `Stores.AppLayout` may be
  undefined (DivScrollY already guards that). Risk: LOW.

## Pattern conformance

- Store `recent*` fields + `loadRecentConversations`/`loadMoreRecent` are a
  1:1 namespaced mirror of the file's existing `page`/`hasMore`/`loadingMore` +
  `loadConversations`/`loadNextPage`. Conforms to [[feedback_match_existing_patterns]].
- Sentinel/observer mirrors `RawCodeView.tsx` + `ConversationPage.tsx`. Conforms.
- Gallery entries mirror `seeded-recent-convos-loading`. Conforms.
- Unit test mirrors `VoiceModel.store.test.ts`; e2e mirrors
  `conversation-list-search.spec.ts` + `sidebar-menu.spec.ts`. Conforms.

## Migration collisions

- None. No migration added (see BASE.md; highest is `...157`).

## OpenAPI regen

- Not required. No backend type/route change; `openapi.json` +
  `api-client/types.ts` untouched. (So the phase-3/8 frontend gates treat this as
  UI work purely because of the `src-app/ui/**` source edits — correct.)

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive state fields mirroring existing paging fields in the same store; no consumer breakage.
- **ITEM-2** — verdict: PASS — new action mirrors `loadConversations`; reuses the existing endpoint + permission gate.
- **ITEM-3** — verdict: PASS — new action mirrors `loadNextPage`; append+dedup guards match existing idioms.
- **ITEM-4** — verdict: PASS — removes a side-effect whose only consumer is migrated in ITEM-6; `/chats` list unaffected (reads `conversations`).
- **ITEM-5** — verdict: PASS — bookkeeping edits preserve existing filter/dedup guards; drops only the now-wrong 20-cap.
- **ITEM-6** — verdict: PASS — sentinel + OS-viewport-root observer is an established repo pattern (`RawCodeView`); RAF-retry covers the defer race.
- **ITEM-7** — verdict: PASS — reuses the existing `Spin`; end-state = render nothing (idiomatic infinite scroll).
- **ITEM-8** — verdict: CONCERN — new render states MUST get gallery cells or `check:state-matrix` fails phase 8; budgeted as its own ITEM + regen step, so tracked, not blocking.
