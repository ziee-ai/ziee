# FIX_ROUND-4 — remediation of the round-3 re-audit

The blind re-audit after FIX_ROUND-3 confirmed the earlier fixes' cores are sound
and found ONE narrow in-flight race. Fixed:

- **MED — stale in-flight loadMore appends onto a reset list** (concurrency):
  `refillRecentIfEmptied`'s in-flight guard made it a no-op when a `loadMore` was
  already fetching, so a delete emptying the list mid-flight let the stale page-N
  response append onto the drained list (empty `seen` set) → a mid-list-only view
  with the most-recent rows missing until the next sync. Fixed with a
  **`recentLoadSeq` epoch**: `loadRecentConversations` captures the epoch and
  discards its result (success OR failure) if it changed mid-flight;
  `refillRecentIfEmptied` bumps the epoch + clears the in-flight flags before
  reloading page 1, so the stale loadMore is dropped and the fresh page-1 reload
  wins. Covered by **TEST-14c** (unit: a deferred in-flight page-2 resolved after
  a mid-flight refill is discarded, not appended).

The reviewer also verified CLEAN: the auto-load↔error gate is non-looping, the
empty-list ErrorState and inline load-more error are mutually exclusive, `setSize`
never under-reports, all hooks precede the first early return, and the selectedId
memo is not a reactive-store loop.

## Also in this round — e2e robustness (test-quality, not product)

The first e2e run exposed that at a tall (900px) viewport the sidebar **eager-loads
all pages on mount** to fill the list, so the specs' "page 2 loads only on scroll"
assumption was wrong (the feature is correct — TEST-6 + TEST-11 passed, proving
initial windowing + scroll-to-oldest + off-screen unmount). Fixed the SPECS (not the
product): a short (480px) viewport so page 1 doesn't fill the list, deterministic
`scrollStep`/`scrollToTop` helpers that wait on real page responses instead of fixed
sleeps, and TEST-10's selector (the row testid is ON the `<button>`, so assert
`aria-current` on the row directly, not `row.locator('button')`).

## Verification

- `npm run check` (ui): PASS. `tsc`: clean. Unit tests: 15/15 PASS.
- e2e: re-run after the spec fixes (see TEST_RESULTS.md).

**New confirmed findings:** 1 (the round-4 re-audit found the createConversation re-anchor omission — see FIX_ROUND-5)
