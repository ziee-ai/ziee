# DECISIONS — all resolved up front

### DEC-1: Does the sidebar reuse the shared `conversations` paging state, or get its own?
**Resolution:** Its own, dedicated `recent*` paging state + `loadRecentConversations`/`loadMoreRecent`, decoupled from the history query.
**Basis:** codebase — `recentConversations` is deliberately kept as the "true most-recent" (unfiltered, `recent`-sort) list, independent of the search/sort-mutable `conversations` used by `/chats`. Sharing state would let a history-page unfiltered page-1 reload reset the accumulated sidebar back to 20 and jump the scroll (a real regression). Independent state mirrors the existing paging fields with zero cross-corruption.

### DEC-2: Trigger idiom — auto infinite-scroll, or a manual "Load More" button?
**Resolution:** Auto infinite-scroll via an `IntersectionObserver` bottom-sentinel (rooted on the `DivScrollY` OverlayScrollbars viewport, `rootMargin:'200px'` prefetch), with a "Loading more" spinner while fetching. No manual button.
**Basis:** user — the sidebar is a nav FEED (infinite-scroll is the specified idiom, explicitly NOT numbered pagination and NOT the `/chats` manual button); codebase — the sentinel-on-OS-viewport pattern is proven in `RawCodeView.tsx` + `ConversationPage.tsx`.

### DEC-3: Page size for the sidebar recent list.
**Resolution:** 20 (reuse the store's existing `limit` field).
**Basis:** convention — the store already uses `limit: 20` for the conversations list; reusing it keeps the sidebar and `/chats` page sizes consistent and avoids a second tunable.

### DEC-4: Is the page size an operational tunable that needs an admin settings row? (Configurable-settings rule)
**Resolution:** FIXED constant — reuse the existing client-side `limit: 20`; NOT admin-configurable.
**Basis:** convention + rationale — page size here is a pure client-side UX/list-window detail with NO server operational implication (not a resource cap, retention, quota, concurrency, or security boundary). The existing `/chats` conversation list page size is likewise a hardcoded client default (`limit` in the store), not a server `*_settings` row. It is already structured as the named `limit` state field (not an inline magic number), so it can be promoted to configurable later without a rewrite. Introducing a `<feature>::settings` table for a sidebar page size would be over-engineering with no operator need. (This is the deliberate "fixed constant with explicit rationale" exception the rule permits.)

### DEC-5: End-of-list + progress affordance — show "Showing N of M" like `/chats`?
**Resolution:** No numeric counter. Show a centered "Loading more" `Spin` (aria-live) while a next page fetches; when exhausted, the list simply ends (no sentinel, no chrome).
**Basis:** convention/idiom — a nav feed signals "more is coming" with a spinner and "that's all" by stopping; the numeric "Showing N of M" belongs to the `/chats` MANAGEMENT view (which keeps it). A count row in a compact sidebar is redundant chrome.

### DEC-6: Cap total accumulated rows / virtualize the sidebar list?
**Resolution:** No cap, no virtualization — accumulate 20-row pages as the user scrolls (initial load bounded to 1 page).
**Basis:** convention — mirrors the `/chats` `ConversationList`, which also renders accumulated rows un-virtualized; sidebar `Menu` rows are lightweight. Virtualization is recorded as a future follow-up if a heavy-user perf finding appears (tracked by the audit's scale-performance angle), not built now — matching precedent and avoiding speculative complexity.

### DEC-7: Does the new sidebar affordance need a new permission?
**Resolution:** No — reuse the existing `Permissions.ConversationsRead` gate.
**Basis:** codebase — the store fetch and the sidebar slot are already gated on `ConversationsRead`; paging the same list introduces no new capability. (⇒ A9/A10 permission-deny tests N/A.)
