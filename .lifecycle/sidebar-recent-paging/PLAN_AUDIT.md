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
- **Virtualization over OS viewport (ITEM-6)** — the exact pattern is proven in
  `kit/table.tsx::VirtualTable`; the OverlayScrollbars `defer` null-viewport race
  is handled by the `events={{ initialized }}` re-render (a state flip forces the
  virtualizer to re-read `getScrollElement`). In the dev gallery
  `Stores.AppLayout` may be undefined (DivScrollY already guards that). Risk: LOW.
- **Shared kit `menu.tsx` edit (ITEM-9)** — extracting the row style into an
  exported class/helper that `Items` then consumes is OUTPUT-IDENTICAL, so the
  Navigation/Tools menus render byte-for-byte the same; their existing gallery +
  e2e coverage is the regression net. This is a sanctioned reuse extraction
  (affordance-parity: "share its logic; never re-derive"), NOT a shared-test-
  harness workaround (B3 N/A — B3 covers `tests/common`/gallery cassette/configs).
  Risk: LOW-MEDIUM (shared component; mitigated by output-identity + coverage).
- **ARIA role change (ITEM-9 / DEC-8)** — switching the sidebar list from
  `role="menu"` to `role="list"` + `aria-setsize`/`posinset` is a deliberate,
  recorded a11y decision (virtualized menu can't honor the menu keyboard
  contract). The a11y audit angle + `gate:ui` axe pass verify no AA/role
  regression. Risk: LOW (correctness IMPROVES for the virtualized case).

## Pattern conformance

- Store `recent*` fields + `loadRecentConversations`/`loadMoreRecent` are a
  1:1 namespaced mirror of the file's existing `page`/`hasMore`/`loadingMore` +
  `loadConversations`/`loadNextPage`. Conforms to [[feedback_match_existing_patterns]].
- Virtualizer mirrors `kit/table.tsx::VirtualTable` (OS-viewport `getScrollElement`
  + `initialized` ready-trigger + fixed `estimateSize` + `overscan`). Conforms.
- Row-style extraction from `menu.tsx` keeps ONE source of truth for the row look
  (affordance-parity/reuse). Conforms.
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
- **ITEM-6** — verdict: PASS — virtualizer over the OS viewport is an established repo pattern (`kit/table.tsx`); `initialized` re-render covers the defer race; last-item watcher is the standard tanstack infinite-scroll trigger.
- **ITEM-7** — verdict: PASS — reuses the existing `Spin`; end-state = render nothing (idiomatic infinite scroll).
- **ITEM-8** — verdict: CONCERN — new render states MUST get gallery cells or `check:state-matrix` fails phase 8; budgeted as its own ITEM + regen step, so tracked, not blocking.
- **ITEM-9** — verdict: CONCERN — touches shared `menu.tsx`; safe only if the extraction is output-identical (verify the Navigation/Tools menu gallery snapshots are unchanged). The `role="menu"`→`role="list"` switch is a recorded a11y decision (DEC-8), verified by the axe/a11y gate. Tracked, not blocking.
