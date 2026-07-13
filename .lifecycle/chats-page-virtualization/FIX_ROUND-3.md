# FIX_ROUND-3 — chats-page-virtualization

Fixed the FIX_ROUND-2 re-audit findings AND the higher-severity bug the REAL-PATH
e2e (TEST-8) caught when it first ran — virtualization silently did nothing on the
real `/chats` page.

## Fixed

- **[HIGH — caught by running TEST-8] nested scrollers → virtualization no-op on
  the real page.** `ChatHistoryPage` wrapped `ConversationList` in its OWN
  `DivScrollY`, and `ConversationList` has its own inner `DivScrollY` around the
  cards. The inner viewport (which the virtualizer attaches to) was therefore
  never height-bounded — its `clientHeight == scrollHeight` — so react-virtual saw
  the whole list as visible and mounted ALL 120 rows (TEST-8 got 120, expected
  < 60; TEST-7's scroll-out assertion also failed). The gallery surface uses a
  single plain bounded scroller, so it PASSED while the real page was broken — the
  precise class of bug only a real-path e2e catches. Fix: removed the redundant
  OUTER `DivScrollY` in `ChatHistoryPage` (replaced with a plain bounded flex
  container), leaving `ConversationList`'s inner scroller as the single, bounded
  viewport the virtualizer windows against. Re-ran: TEST-7 + TEST-8 now PASS
  (mounted ≪ 120), regression specs (load-more, search) PASS.

- **[re-audit] estimator stacked/inline branch reverted to inline-only** (the
  false-premise finding). `ConversationCard`'s `flex-col sm:flex-row` is a VIEWPORT
  media query; the virtualized path runs ONLY at a ≥ sm viewport (a < sm viewport
  → `nativeScroll` → the plain path, which never calls the estimator), so the card
  is ALWAYS inline where the estimate is used. Removed `SM_BREAKPOINT` /
  `META_ROW_HEIGHT` and the content-width branch; the estimator models the inline
  layout only, with a comment explaining why. This also removed the over-estimate
  the branch introduced at the narrow gallery surface.

- **[re-audit] TEST-12 premise corrected.** Its "where ConversationCard stacks its
  meta" claim was false (desktop viewport → inline even in a 390px container).
  Reworded to what it actually proves: a narrow (390px) content COLUMN still
  windows rows and stays jank-free at rest (responsive-fidelity). The BOUNDARY-
  title unit tests still hold with the inline-only estimator (width-sensitivity
  comes from the title wrapping sooner at a narrow content width), so no further
  test change was needed.

## Re-audit

A FOURTH full blind round (fresh diff-only agent over the FIXED diff, incl. the
ChatHistoryPage scroller change) was run. Result recorded below.

**New confirmed findings:** 0
