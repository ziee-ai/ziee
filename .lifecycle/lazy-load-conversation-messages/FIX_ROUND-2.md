# FIX_ROUND-2 — the 3 findings surfaced by FIX_ROUND-1's re-audit

## Fixed

- **E1 (state, med)** Wired the scroll-DOWN direction: added a bottom-load
  IntersectionObserver (`bottomLoadSentinelRef`, 800px bottom rootMargin) that
  calls `Stores.Chat.loadNewerMessages()` when `hasMoreAfter && !isStreaming`,
  and made `jumpToLatest` snap to the real tail (`loadMessages`) before scrolling
  when `hasMoreAfter`. So after an around= jump (find / `#message-` deep-link /
  cancelEdit of an older message) the user can now page forward to newer messages
  ("load more around the found message" in the down direction) and "Jump to
  latest" reaches the actual latest. No anchor restore needed for append (content
  added below the fold doesn't shift the visible region).
- **E2 (correctness, low)** `build_snippet` now computes the match offset as
  `hay[..byte_idx].chars().count().min(chars.len())` over the LOWERCASED string
  (a valid char boundary) instead of slicing the ORIGINAL `collapsed` at a foreign
  byte offset — eliminating the panic / mis-cut on length-changing lowercasing
  (Turkish `İ`→`i̇`, etc.). Guarded by a new unit test
  (`snippet_does_not_panic_on_length_changing_lowercasing`).
- **F1 (a11y, low)** The MessageList "loading older" `aria-live="polite"` region
  is now ALWAYS mounted (only the spinner toggles inside it), matching the
  proven-correct find-bar structure, so screen readers announce the load.

## Verification

- `cargo test -p ziee --lib chat::core::types::message::tests` → 7/7 (incl. the new
  Unicode snippet guard).
- `npx tsc --noEmit` (ui) clean; `npm run check` (ui) green (state-matrix +
  testid-registry regenerated for the changed conditionals).
- A fresh blind round-3 verifier reviewed the diff (angles: correctness,
  concurrency, state-management, a11y), scrutinizing the E1 bottom-sentinel
  loop/viewport-yank risk, `loadNewerMessages` wiring, the E2 offset safety, and
  the F1 live-region structure. It cleared E2/F1/loadNewer-merge as correct but
  found two issues on the newly-wired scroll-down (carried into FIX_ROUND-3):

  - **G1 (state, med)** bottom-load + smooth-follow could cascade an
    un-interruptible auto-scroll to the tail when the user sits at the bottom of a
    mid-conversation window (`isAtBottom && hasMoreAfter`).
  - **G2 (concurrency, low)** `loadNewerMessages` lacked the in-flight guard
    `loadOlderMessages` has → the bottom sentinel could fire two concurrent
    same-cursor fetches.

**New confirmed findings:** 2
