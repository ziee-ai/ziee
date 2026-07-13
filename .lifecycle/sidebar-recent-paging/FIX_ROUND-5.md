# FIX_ROUND-5 — remediation of the round-4 re-audit

The blind re-audit after FIX_ROUND-4 confirmed the epoch mechanism is sound and
cleared concurrency / widget-error-exclusivity / a11y. It found ONE remaining
consistency bug:

- **MED — `conversation.created` omitted the recentPage re-anchor**
  (state-management): the created handler prepends + bumps `recentTotal` but was
  the one front-mutating path that did NOT re-anchor `recentPage` (syncRecentFront,
  deleteConversation, bulkDelete, sync-delete all do). Creating ≥`limit` chats
  locally leaves `recentPage=1` while the list grows, so the next `loadMoreRecent`
  fetches a fully-overlapping server page → `noProgress` → older pages stranded.
  Fixed: the created handler now re-anchors `recentPage=floor(length/limit)` on a
  genuine new prepend. Covered by **TEST-5** (added `recentPage` assertion).

## Verification

- `npm run check` (ui): PASS. `tsc`: clean. Unit tests: 15/15 PASS.

**New confirmed findings:** 1 (the round-5 re-audit found a delete-vs-in-flight-loadMore skip — see FIX_ROUND-6)
