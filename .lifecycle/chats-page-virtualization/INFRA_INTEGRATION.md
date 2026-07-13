# INFRA_INTEGRATION — chats-page-virtualization

Two mandatory Phase-5 walks per item: (1) user-experience walk, (2)
infrastructure-integration walk (every subsystem the item touches).

## UX walk (how a real user encounters each item)

- **ITEM-3/4/5 (the visible change):** User opens `/chats` with hundreds of chats.
  They scroll fluidly; the thumb doesn't jump; every card is fully drawn; click
  opens the chat; hover reveals delete; multi-select + bulk-delete work; Load-More
  fetches the next page; search filters. Nothing about virtualization is
  perceptible — that IS the success criterion. On a phone (`nativeScroll`) the
  page scrolls natively and renders the loaded set plainly (no regression).
- **ITEM-1/2/6 (invisible machinery):** the estimator + measured-height seed make
  the FIRST scroll of a freshly-opened `/chats` land rows at true height (no
  correction wobble). The DEV metrics are invisible to users (dev/e2e only).
- **ITEM-7 (gallery):** developer/design-critic surface only — not user-facing.

## Infra-integration walk (subsystems each item touches)

- **Scroll subsystem (OverlayScrollbars / `DivScrollY` / `AppLayout.nativeScroll`):**
  the virtualizer's scroll element is the inner card `DivScrollY`'s OS viewport.
  Must handle: (a) viewport not ready on first render → `scrollerReady` gate +
  null-root fallback (plain path); (b) `nativeScroll` mobile → no OS instance →
  plain render; (c) the OS instance is recreated on some layout changes → key the
  observer/virtualizer readiness on `scrollerReady`. Mirrors ConversationPage.
- **ChatHistory store (READ-ONLY here):** `conversations` array identity changes on
  every load/append/prepend/delete/sync. The virtualizer keys rows by `id`
  (`getItemKey`), so index shifts (prepend of a new conversation via
  `conversation.created`; append via Load-More; removal via delete/sync) keep
  measured heights and don't teleport. `total`/`hasMore` drive the footer. No
  store mutation → the `sync:conversation`, `conversation.created`,
  `conversation.titleUpdated`, bulk-delete, and cross-device-delete paths are all
  untouched and keep working.
- **Selection / bulk-delete:** `selectedIds` is a Set read reactively; toggling
  selection re-renders ConversationList. Under virtualization only windowed rows
  re-render (memoized row site, DEC-9) — selecting still updates the checkbox on
  visible rows; "Select All" operates on the full `conversations` array (store
  action, not DOM), so it is unaffected by which rows are mounted. Bulk-actions
  bar stays ABOVE the scroller (outside virtual math).
- **Search portal:** `getSearchBoxContainer` portal is rendered by ConversationList
  OUTSIDE the virtual container — untouched. A no-match search yields an empty
  `conversations` → the existing empty-arm renders (not the virtual container), so
  virtualization is bypassed when there are 0 rows.
- **Per-row extensions (`ConversationCard` trailing / `conversationHref`):** the
  card is rendered unchanged inside each virtual row, so the chat-extension
  registry hooks (project "Remove from project" trailing, href override) still
  fire. Lazy-on-hover trailing still works (hover is per mounted row).
- **Routing / navigation:** clicking a card calls `navigate(...)` — unchanged.
  Scrolling away unmounts a row mid-navigation is impossible (navigation is
  synchronous on click of a mounted row).
- **Keyboard / a11y / focus:** each row keeps `role="button"`/`tabIndex`. Off-window
  rows are unmounted → tab order spans mounted rows only (same tradeoff as
  MessageList; no NEW a11y violation — Phase-6 a11y angle verifies). `overflow-
  anchor: none` is NOT needed here (no reverse-prepend anchor restore; virtual-core
  handles prepend via stable keys).
- **Gallery / visual-test harness:** new surface registered in `seededSurfaces.tsx`
  + coverage/state/testid/overlay registries (`npm run check` gates). Backend-free
  seeding via `ChatHistory.store.setState`. Mirrors `seeded-chat-history-list` +
  `MessageListLongDemo`. Must add a POPULATED + a 390px NARROW state (checklist).
- **Build / tree-shaking:** DEV metrics behind `import.meta.env.DEV`; estimator +
  cache are pure modules. No new runtime dep (react-virtual already present). No
  cargo/proc-macro surface (frontend-only) → B4 clean-build risk N/A.
- **Merge/live4:** only `ConversationList.tsx` overlaps; keep the edit a single
  localized region; `ChatHistory.store.ts` untouched.

**No gap found that changes the plan.** The one design nuance surfaced: when
`conversations.length === 0` the existing empty/error arm renders (virtual
container not mounted), so the scroller-ref/virtualizer must tolerate a
0-count/absent scroll element without throwing — handled by the `scrollerReady` +
null-root guards (plain fallback), same as MessageList's `count===0` early return.
