# FIX_ROUND-1 — disposition of the phase-6 blind-audit findings

3 blind reviewers (12 angles). Reviewer C: 0 findings (all clean). One HIGH, two
MEDIUM, three LOW across A/B. All confirmed findings fixed; one accepted as an
inherent virtualization tradeoff.

## Fixed

- **A1 (correctness, HIGH)** On the mobile native-scroll path there is no inner
  OS viewport (window scroll), so `getScrollElement` returned null → the
  virtualizer never observed scroll → only overscan rows rendered (blank/missing
  on mobile). FIX: virtualize ONLY on the desktop inner-scroll path; on native
  (`nativeScroll`) render the lazy-load-bounded window PLAINLY with a DOM-based
  scroll/anchor fallback. The `virtualize` prop is keyed on the stable
  `nativeScroll` flag (NOT scroll-element readiness) so it never flips
  mid-session and thrashes the layout.
- **A2 + A4 + B3 (concurrency/a11y, MED/LOW)** The `scrollToMessageId` re-assert
  rAF loop wasn't cancelled across overlapping calls (rapid find-next fought),
  had no unmount guard, and no user-gesture cancellation. FIX: a single
  cancellable re-assert (`reassertRef`) — cancelled on a new call, on unmount
  (`useEffect` cleanup), and on a real user gesture (wheel/touch/keydown, which
  don't fire for our own programmatic scroll).
- **A3 (correctness, MED)** The initial bottom-jump landed on ESTIMATED row
  heights (rows unmeasured) with no re-follow, so the conversation could open not
  pinned to the latest. FIX: `scrollToBottom()` (`scrollToIndex(count-1,'end')` +
  re-assert) called from the initial-jump so it settles on the measured bottom.
- **B1 (tests-quality, MED)** `indexOfMessageId` was added + unit-tested but
  `MessageList` inlined `findIndex`, so TEST-1 covered dead code. FIX:
  `indexOfMessageId` is now array-based and actually used by the imperative
  handle (scrollToMessageId + restoreAnchor); TEST-1 updated to the array form.

## Also fixed (surfaced while validating the fixes via e2e)

- **captureAnchor flaky teleport** — the original capture read `virt.scrollOffset`
  (and then the DOM), both of which LAG the actual `scrollTop` right after a
  programmatic scroll, yielding a stale/wrong anchor → a full-height teleport on
  prepend (intermittent). FIX: capture from the REAL `scrollTop` +
  `virt.getVirtualItemForOffset(scrollTop)` (public API; measurement-based,
  render-independent). Verified stable across repeated e2e runs.

## Accepted (inherent virtualization tradeoff — won't fix)

- **B2 (a11y, LOW)** Unmounting off-screen rows drops keyboard focus to `<body>`
  when a focused in-message control scrolls out, and native Ctrl+F / select-all
  cover only mounted rows. This is intrinsic to ANY virtualized list. Mitigation
  already present: the in-app find (Cmd/Ctrl-F → `ConversationFindBar`) is
  SERVER-SIDE and covers ALL messages (not just mounted), and jumps to matches
  via the virtualizer; reading order (DOM order = visual order) is preserved.
  Accepted as documented behavior.

## Re-audit

A fresh blind round (correctness/concurrency/state-management/a11y over the
current diff) verified every fix correct and found no fix-introduced regression.
It confirmed: the two-path handle agrees on both paths; the re-assert loop is
correctly cancelled (new-call/unmount/gesture, bounded to 3 frames, no leak, no
self-cancel on programmatic scroll); `captureAnchor` via
`getVirtualItemForOffset(real scrollTop)` is pixel-exact and its offset cancels
between capture and restore; `scrollToBottom` hits the true DOM bottom with no
loop and doesn't fight the follow; `indexOfMessageId` is correct + used.

One residual (rejected):

- **scrollMargin (a11y/correctness, low, PLAUSIBLE — rejected)** The virtualizer
  treats item 0 as content-y 0, but the list actually starts ~16px down (the
  `pt-4` container + the always-mounted loading-older/top-sentinel siblings), so
  a jumped/found message centers ~16px below true center. REJECTED as cosmetic:
  the offset is ~16px and imperceptible; the reviewer proved the anchor
  capture/restore is pixel-exact (the constant offset cancels) and
  `scrollToBottom` reaches the true bottom. A `scrollMargin` fix would have to
  track a VARIABLE-height header region (the loading-older spinner is 0px idle,
  ~40px while fetching), so hardcoding it is brittle and a dynamic measure risks
  new jitter for zero perceptible gain. Not a confirmed defect.

**New confirmed findings:** 0
