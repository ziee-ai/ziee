# FIX_ROUND-1 — tool-artifact-grouping (follow-up #3)

Merged the phase-6 ledger (3 blind angles = 12 dimensions), fixed the confirmed
findings, then re-audited the delta.

## Fixes applied (to the new ConversationPage scroll effect)

1. **Mobile no-op** (LEDGER: edge-cases/regressions — MEDIUM, confirmed). On the mobile
   plain path (`nativeScroll`, non-virtualized) `messageListRef.scrollToBottom()` does
   nothing, so an off-bottom approval was never scrolled to on mobile. Fixed by splitting
   on `nativeScroll`: desktop → `scrollToBottom()` (moves the virtualized OverlayScrollbars
   viewport), mobile → `messagesEndRef.scrollIntoView({behavior:'auto'})` (moves the native
   document scroll). NOTE: calling BOTH on desktop makes the anchor jump fight the
   virtualized scroll and leaves the view short (caught by the e2e — fixed by the split).

2. **Missing sibling guards** (LEDGER: regressions — MEDIUM ×2). The effect dropped the
   auto-follow's `!pendingAnchorRef.current` + `!hasMoreAfter` guards, so an approval during
   an in-flight older-page prepend (janky double-scroll) or a mid-list `hasMoreAfter` window
   (a `scrollToBottom` to the loaded-slice tail re-entering the bottom-load sentinel) could
   misbehave. Added both — the effect now mirrors the auto-follow exactly, EXCEPT the
   intentional `isAtBottom` bypass (documented). `hasMoreAfter` added to the deps.

3. **Cross-conversation spurious scroll** (LEDGER: regressions/edge-cases/api-contract —
   MEDIUM, flagged by all 3 angles). `toolCalls` is a process-global map that is never
   cleared and carries no conversation id. Two fixes: (a) a mount-seed effect marks
   already-pending approvals as seen so a fresh page mount isn't yanked by a leftover from a
   previously-viewed conversation; (b) the main effect records EVERY newly-pending approval
   as seen BEFORE the guard block, so an approval that arrives while a guard is active is not
   left un-seen (which could scroll a same-mount A→B switch later). Only the safe path scrolls.

## Accepted-by-design / documented (not defects)

- **perf** (MEDIUM): ConversationPage now re-renders on every `toolCalls` mutation. It
  already re-renders on every streaming delta during a stream, the read is the idiomatic
  `McpToolUseRenderer` pattern, and the `scrolledApprovalsRef` Set is bounded by the small
  tool-call count. Download-progress ticks are the only new source and are rare.
- **a11y** (MEDIUM): the scroll moves the viewport but doesn't focus/announce the approval
  (no aria-live). Pre-existing on the approval card (not a regression); moving focus would
  steal it. A broader a11y enhancement, out of this fix's scope.
- **test negative-control**: the e2e asserts `toBeInViewport` without an in-test
  out-of-viewport baseline — but I ran an EXTERNAL negative check (disabled the scroll,
  rebuilt) and confirmed the test FAILS on `toBeInViewport` (approval below the fold), so it
  genuinely exercises the fix.
- Unbounded `scrolledApprovalsRef` growth / no re-scroll on POST-failure re-pend: bounded /
  acceptable (the user just interacted with a re-pend).

## Re-audit

A fresh blind agent reviewed the fix delta. It confirmed the mount-seed, added guards, and
dual-scroll pattern are sound and raised one residual MEDIUM (an approval arriving during a
guarded window in conversation A being left un-seen → same-mount A→B stray scroll) — **fixed**
by recording seen BEFORE the guard (item 3b). The scroll test passes with the fix and fails
without it (external negative check).

**New confirmed findings:** 0
