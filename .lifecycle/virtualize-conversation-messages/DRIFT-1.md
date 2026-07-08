# DRIFT-1 — implementation vs plan (virtualize-conversation-messages)

- **DRIFT-1.1** — verdict: resolved — ITEM-2's `scrollToMessageId` needed a
  short re-assert loop (`scrollToIndex` re-called over ~3 frames), not planned.
  Reason surfaced in e2e: after a jump to an EARLY message the stale scroll
  clamps to the bottom, the bottom-load sentinel fires `loadNewer`, and that
  window change + estimate→measured settling defeats a single `scrollToIndex`.
  The re-assert is a correctness refinement WITHIN ITEM-2's intent (scroll a
  loaded message into view); no plan change.
- **DRIFT-1.2** — verdict: resolved — the lazy-load e2e specs (which double as
  the virtualization regression guard, TEST-4/5/6) needed virtualization-aware
  assertions: a loaded-but-OFF-SCREEN message is no longer in the DOM, so
  `lazy-load-branch-reset` force-scrolls to the top (via the always-mounted top
  sentinel) before asserting the oldest message renders, and the new
  `virtualize-messages` spec scrolls via the top sentinel rather than a raw
  `scrollTop=`. These are TEST adaptations to the (intended) DOM change, not
  product drift.
- **DRIFT-1.3** — verdict: impl-wins — `MessageList`'s `getScrollElement` /
  `scrollerReady` were made OPTIONAL (default `() => null` / `false`) rather than
  strictly required, so the standalone gallery empty-state surface
  (`seeded-message-list-empty`, which renders `<MessageList/>` with no props)
  keeps working without a scroll container. Plan (ITEM-1/6) implied required
  props; the optional-with-safe-default form is strictly more robust and the real
  caller (ConversationPage) always passes them. PLAN intent (virtualized render
  driven by the page's viewport) is unchanged.
- **DRIFT-1.4** — verdict: none — per DEC-8 the element-measurement
  `scrollAnchor.utils` pure fns (`pickTopAnchor`/`restoreDelta`) stay exported +
  tested even though `ConversationPage` no longer imports them (the anchor is now
  index-based via the virtualizer). Intentional.
- **DRIFT-1.5** — verdict: none — the sentinels + `messagesEndRef` initial-jump/
  bottom-follow were KEPT unchanged (DEC-5); the preserved `getTotalSize()` scroll
  geometry means they work as-is under virtualization, confirmed by the passing
  anchor / sse / branch-reset e2e. As planned.
- **DRIFT-1.6** — verdict: none — the index-based prepend anchor (ITEM-4) uses
  `getOffsetForIndex` + `scrollToOffset` + the virtualizer's
  `shouldAdjustScrollPositionOnItemSizeChange`, exactly per DEC-4; the
  scrollTop/scrollHeight anchor e2e (TEST-5) confirms no teleport. As planned.

**Unresolved drifts:** 0
