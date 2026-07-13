# FIX_ROUND-3 — remediation of the round-2 re-audit

The blind re-audit after FIX_ROUND-2 surfaced three more delete/failure edge cases
(the reviewer confirmed the earlier fixes' cores are sound). All fixed:

- **MED — delete-drains-to-empty strands the sidebar** (correctness): deleting all
  loaded rows while more exist server-side left `recentConversations=[]` → the
  Empty render has no virtual rows → the last-item auto-load effect can never fire
  → "no conversations" shown though rows remain. Fixed: `refillRecentIfEmptied()`
  reloads page 1 from delete / bulkDelete / sync-delete when the loaded list drains
  empty while `recentHasMore`. Covered by **TEST-14 / TEST-14b**.

- **MED — load-more failure unrecoverable when the page fits the viewport**
  (state-management): the scroll-to-clear recovery could never run if the loaded
  page fit the viewport (no scrollback), and there was no visible load-more error,
  so paging silently stuck. Fixed: replaced the fragile scroll-clear with an
  explicit visible **"Couldn't load more · Retry"** affordance
  (`chat-recent-loadmore-error` / `-retry`); the auto-load effect now only gates on
  `!recentError`. Covered by **TEST-13** (retry leg).

- **LOW — aria-setsize from a drifted recentTotal** (a11y): fixed with
  `setSize = recentHasMore ? max(recentTotal, length) : length` so a fully-loaded
  list announces the exact count and never under-reports.

The reviewer explicitly verified CLEAN: the retry cycle does not hammer the API
(one round-trip per deliberate retry), the empty-list first-load ErrorState
persists, and the `recentPage=floor(length/limit)` re-anchor is sound against
skip/infinite-overlap across create/delete/bulkDelete/syncRecentFront (the loaded
set is always a contiguous server prefix).

## Verification

- `npm run check` (ui): PASS. `tsc`: clean. Unit tests: 14/14 PASS.

**New confirmed findings:** 1 (the round-3 re-audit found one narrow in-flight
race — see FIX_ROUND-4)
