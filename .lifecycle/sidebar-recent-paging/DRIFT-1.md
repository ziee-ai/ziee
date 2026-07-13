# DRIFT-1 — implementation vs plan

- **DRIFT-1.1** — verdict: impl-wins — `sync:conversation` CREATE path. The plan
  said "reload recent page 1" (`loadRecentConversations(1)`, a REPLACE). While
  wiring TEST-9 I found a REPLACE would collapse an infinite-scrolled sidebar back
  to one page and jump the scroll — the opposite of the feature's intent.
  Re-implemented as a dedicated `syncRecentFront()` action that fetches page 1 and
  MERGE-prepends only the not-yet-seen rows, preserving the accumulated pages.
  PLAN ITEM-5 amended; TESTS gains TEST-5b (unit) + TEST-9 (e2e) covers it.
  `sync:reconnect` deliberately KEEPS the full page-1 replace (a fresh view after
  an offline gap is correct).

- **DRIFT-1.2** — verdict: resolved — `enableMapSet()` was not called by the
  ChatHistory store despite it mutating `selectedIds` (a Set) via immer; it worked
  in-app only because another store's module import ran `enableMapSet()` first.
  Added `enableMapSet()` to the store (idempotent). Not a plan ITEM (a latent-bug
  fix surfaced by the unit test), so no plan amendment — recorded here.

- **DRIFT-1.3** — verdict: resolved — the widget rewrite changed the gallery
  state-matrix's detected required states for the widget from `delayed`/`open` to
  `empty`/`open` (the loading branch is now `!recentInitialized`, not an async
  "delayed" signal). Regenerated the generated gallery/testid files and reconciled
  `stateCoverage.ts` (removed the stale `:delayed` entry, kept `:open`, added
  `:empty`). Expected consequence of the rewrite, not a design change.

- **DRIFT-1.4** — verdict: resolved — testid continuity. The plan implied the row
  list is a new surface; implementation PRESERVES the existing row navigate testid
  `chat-recent-conversations-menu-item-<id>` (required by the `conversation-sync`,
  `chat-conversation-sync`, and `assistant-switching` specs) and the kebab testids,
  and only renames the container testid to `chat-recent-conversations-list` (used
  by no existing spec). No behavior drift.

**Unresolved drifts:** 0
