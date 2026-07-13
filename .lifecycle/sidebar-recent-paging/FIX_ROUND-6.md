# FIX_ROUND-6 — remediation of the round-5 re-audit

The blind re-audit after FIX_ROUND-5 confirmed the recentPage invariant is now
consistent across all synchronous paths and the epoch discards drain-to-empty
supersessions correctly. It found ONE remaining in-flight race:

- **MED — delete concurrent with an in-flight loadMore skips a row**
  (concurrency): a delete that runs while a `loadMore(page N)` is in flight
  shrinks the list + re-anchors `recentPage`, but the pending page-N response
  (not epoch-superseded because the list wasn't drained to empty) resolved and
  overwrote `recentPage = targetPage` — the stale fetched page — reverting the
  re-anchor, so the next `loadMore` fetched a shifted offset that skipped a server
  row until a page-1 replace healed it. Fixed by unifying the append path with
  every other mutation: on resolve, re-anchor `recentPage = floor(length/limit)`
  (never the stale `targetPage`), so the offset always stays ≤ length and
  consecutive fetches overlap (dedup) with no gap. Covered by **TEST-14d** (unit:
  a deferred in-flight page-3 that resolves after a mid-flight delete re-anchors
  to floor(59/20)=2, not the stale 3).

This unification also means `recentPage` is now derived by exactly one rule
(`floor(loaded-length / limit)`) at every mutation site — load-append,
syncRecentFront, all three delete paths, and conversation.created — which is why
the re-audits converged: there is no longer a path that can desync the cursor.

## e2e

Also fixed TEST-9's final assertion (it looked for a *middle* row after scrolling
to the bottom, where virtualization unmounts it) to instead prove the loaded pages
survived a create by scrolling all the way to the oldest.

## Verification

- `npm run check` (ui): PASS. `tsc`: clean. Unit tests: 16/16 PASS.
- e2e: 6/7 passed on the prior run (TEST-9 assertion fixed here); full re-run recorded in TEST_RESULTS.md.

**New confirmed findings:** 0 (verified by the round-6 re-audit)
