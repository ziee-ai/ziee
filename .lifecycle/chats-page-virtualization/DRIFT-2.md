# DRIFT-2 — implementation vs plan (round 2, during phase 6-7)

- **DRIFT-2.1** — verdict: impl-wins — **ChatHistoryPage's redundant outer
  scroller removed (ITEM-4 extension).** The plan scoped the scroll-element wiring
  to `ConversationList` only. The real-path e2e (TEST-8) revealed that
  `ChatHistoryPage` wrapped `ConversationList` (which owns its own inner
  `DivScrollY`) in a SECOND `DivScrollY`, so the inner viewport the virtualizer
  attaches to was never height-bounded → react-virtual mounted every row
  (virtualization was a silent no-op on the real page). Removing the redundant
  outer scroller (a plain bounded flex container in its place) is required for
  ITEM-3/4 to function. `ChatHistoryPage.tsx` added to PLAN "Files to touch". This
  does NOT overlap live4 (which edits the sidebar + store, not ChatHistoryPage).
  Resolved.

- **DRIFT-2.2** — verdict: impl-wins — **estimator reverted to inline-only.** A
  FIX_ROUND-2 attempt to model the stacked (< sm) card layout was based on a false
  premise (the card's `sm:` is a VIEWPORT media query, and the virtualized path
  only runs at a ≥ sm viewport). Reverted; the estimator models the inline layout
  only, matching where it is actually used. DEC-1 unaffected (still a content-aware
  inline estimate mirroring `estimateMessageHeight`). Resolved.

**Unresolved drifts:** 0
