# FIX_ROUND-3 — the 2 findings surfaced by FIX_ROUND-2's re-audit

## Fixed

- **G1 (state, med)** The bottom-load sentinel could cascade an un-interruptible
  auto-scroll to the real tail: when the user sat at the bottom of a
  mid-conversation window (`isAtBottom && hasMoreAfter`), appending a newer page
  fired the smooth-follow effect (suppressed only by `pendingAnchorRef`, null for
  appends), whose re-scroll re-entered the 800px bottom sentinel → re-fired
  `loadNewerMessages` → repeated to the tail. FIX: the smooth-follow effect is now
  additionally gated on `!hasMoreAfter` (added to its deps). Auto-follow only runs
  when the loaded window actually holds the newest message — so it still follows
  live streaming turns (which run at the tail, `hasMoreAfter=false`) but never
  cascades a mid-conversation forward-pagination.
- **G2 (concurrency, low)** `loadNewerMessages` now has a `loadingNewer`
  re-entrancy guard (mirroring `loadingOlder`): set true at start, cleared on
  success/error, and reset at every window-reset point (`loadMessages`,
  `jumpToMessage`, the `reconcileTail` reset branch, the A→B conversation-switch
  block, and `reset()`), so a mid-fetch conversation switch can't leave it stuck.

## Verification

- `npx tsc --noEmit` (ui) clean; `npm run check` (ui) green (state-matrix
  regenerated for the changed smooth-follow condition).
- A fresh blind round-4 verifier reviewed the diff (angles: correctness,
  concurrency, state-management), specifically checking that the `!hasMoreAfter`
  gate doesn't suppress legitimate streaming auto-follow and that `loadingNewer`
  is cleared on every path. It reported **no findings** — both fixes correct, no
  regression. It noted one benign asymmetry (the `loadConversationState`
  cache-restore set omitted `loadingNewer: false`, though the A→B switch block
  clears it first) — added `loadingNewer: false` there too as a defensive
  symmetry (strictly sets a guard to false on restore; needs no re-audit).

**New confirmed findings:** 0
