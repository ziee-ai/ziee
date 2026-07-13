# INFRA_INTEGRATION — the two mandatory walks (Phase 5)

## (1) User-experience walk — how a real user encounters each item

- **Open the app / sidebar** — on mount the widget calls
  `loadRecentConversations()` (ITEM-2/6); the user sees the initial spinner then
  their newest ~20 chats. Unchanged from before for a light user.
- **Scroll the sidebar** — as the last virtual row nears the end, the next page
  auto-loads (ITEM-3/6) with a "Loading more" spinner (ITEM-7); the user keeps
  scrolling seamlessly to older chats. At the oldest chat the list stops (no
  spinner, no button) — the user sees the list end.
- **Heavy user (thousands of chats)** — only a window of rows is in the DOM
  (virtualization, ITEM-6), so scrolling stays smooth; memory/layout is O(viewport).
- **New chat (this window)** — `conversation.created` prepends it to the top;
  loaded older pages stay (ITEM-5). **New chat (other device)** — the SSE
  `sync:conversation` create merge-prepends it (`syncRecentFront`, ITEM-5)
  without collapsing the scrolled-in list.
- **Delete a chat** — from the row kebab (unchanged `ConversationRowActions`); the
  row disappears and the paging counter stays honest (ITEM-5) so `recentHasMore`
  doesn't desync.
- **Open a chat** — clicking a row navigates; the open row shows `aria-current`
  (ITEM-9). A keyboard user Tabs through the row buttons (list semantics, DEC-8).

## (2) Infrastructure-integration walk — subsystems each item touches

- **Sync (SSE notify-and-refetch)** — the sidebar list is refreshed by
  `sync:conversation` (create → `syncRecentFront` merge-prepend; delete → prune +
  `recentTotal--`) and `sync:reconnect` (full page-1 replace). Verified the perm
  self-gate (`hasPermissionNow(ConversationsRead)`) is preserved on every recent
  fetch (no-403 rule). The refetch endpoint (`GET /api/conversations`) is the same
  permission-gated route → audience/perm parity holds.
- **Permissions/authz** — no new permission; reuses `ConversationsRead` (already
  on the store fetch + the sidebar slot). No migration, no `permissions.rs` change
  ⇒ A9/A10 N/A. The recent loader early-returns without a request when the perm is
  absent (unit TEST-1b).
- **Chat pipeline / conversation lifecycle** — the `/chats` history list
  (`loadConversations` → `conversations`) is now fully DECOUPLED from the sidebar
  (`recentConversations`); a history search/sort or page-1 reload no longer
  touches the sidebar (ITEM-4, unit TEST-4). Confirmed the only reader of
  `recentConversations` is this widget.
- **Extensions registry** — row href resolution still routes through
  `chatExtensionRegistry.conversationHref` (projects override etc. unchanged); the
  row kebab still hosts `useConversationMenuContributions` overlays. The row
  navigate testid `chat-recent-conversations-menu-item-<id>` and the kebab testids
  (`chat-recent-row-actions-btn-<id>`, `chat-recent-row-menu-<id>`) are PRESERVED,
  so the projects `sidebar-menu` spec + the `conversation-sync` / `assistant-
  switching` specs keep passing.
- **Kit `<Menu>` (shared)** — the row styling is now sourced from the exported
  `menuRowClasses` (ITEM-9); `Items` consumes it output-identically, so the
  Navigation + Tools sidebar menus render unchanged. Verified via tsc + the
  kit-manifest/testid regen; the gallery snapshots for those menus are the
  regression net (Phase 8 gate:ui).
- **Gallery / state-matrix** — the widget rewrite changed its detected states
  (delayed→ gone; empty/open required); regenerated `stateMatrix.generated.ts`,
  `galleryCoverage.generated.ts`, `testIds.generated.ts` and reconciled
  `stateCoverage.ts` (empty + open entries). Added dedicated seeds (loaded /
  loading-more) so `gate:ui`/runtime-health exercise the new render paths incl. a
  narrow (390px) viewport.
- **OverlayScrollbars / virtualizer** — the virtualizer roots on the DivScrollY OS
  viewport via `getScrollElement`; the `events={{ initialized }}` re-render handles
  the OS `defer` null-viewport race (mirrors `kit/table.tsx`). No change to
  `DivScrollY` itself (it already forwards `ref` + `events`).
- **Immer MapSet** — the store mutates a `Set` (`selectedIds`) through immer but
  never called `enableMapSet()` (worked in-app only because another store's import
  ran first). Made the store own it — a latent-bug fix surfaced by the unit test.
- **Realtime/streaming/workflow/notifications** — not touched (the sidebar recent
  list is orthogonal to message streaming and workflow runs).
