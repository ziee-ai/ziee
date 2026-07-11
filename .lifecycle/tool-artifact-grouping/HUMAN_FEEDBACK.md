# HUMAN_FEEDBACK — tool-artifact-grouping (follow-up #3)

The lead's feedback that PROMPTED this round is recorded and resolved:

- **FB-1** [status: resolved] — "the approval-scroll from #134 does NOT actually work in
  the real app — it calls scrollIntoView but the chat list is virtualized inside an
  OverlayScrollbars viewport, so a per-element scrollIntoView is a no-op, and your e2e only
  asserted scrollIntoView was CALLED (not that it scrolled)" → Replaced the dead
  `containerRef.scrollIntoView` with a scroll via the app's own virtualization-aware
  `messageListRef.scrollToBottom()` (mobile falls back to the end-anchor), driven from
  `ConversationPage` when a new `pending_approval` appears, BYPASSING the `isAtBottom`
  auto-follow gate that suppressed it. Rewrote the e2e to reproduce the real below-the-fold
  scenario and assert `toBeInViewport` — and VERIFIED it fails without the fix (external
  negative check: disabled the scroll, rebuilt → the test goes red on `toBeInViewport`).
  [generalizable: yes — a UI e2e must assert the EFFECT (element in viewport / DOM state),
  never that a scroll/DOM API was merely CALLED; and confirm a behavior test fails when the
  fix is reverted, so it can't false-green.]

No further human feedback received on this round; it has not yet been reviewed against the
running app. Suggested live check (deferred if the stack isn't bindable): a long
tool-calling answer with the user scrolled up — when the tool-call blocks appear asking
permission, the view smoothly jumps to the approval.
