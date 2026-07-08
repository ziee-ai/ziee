# TESTS — virtualize-conversation-messages

Virtualization is a rendering change over the lazy-load window, so the strongest
guard is that the lazy-load e2e specs (anchor / jump / find) STAY GREEN under the
virtualized code — they now double as virtualization regression tests — plus a
new spec proving the DOM is actually virtualized, and pure unit tests for the new
index-mapping + index-anchor math. No cosmetic tests — the e2e drives the real
virtualizer against real scroll geometry.

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/core/stores/messageWindow.test.ts` — asserts: a pure `indexOfMessageId(messages, id)` helper returns the correct window index for a loaded id and `-1` for an unloaded id (the id→index mapping behind `scrollToMessageId`).
- **TEST-2** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.test.ts` — asserts: the pure `indexRestoreOffset(offsetForIndex, viewportOffset)` returns the scrollOffset that re-pins the anchor index at its captured viewport offset (and clamps ≥ 0), i.e. the index-based analog of `restoreDelta`.

## E2E

- **TEST-3** (tier: e2e) [covers: ITEM-1, ITEM-6] file: `src-app/ui/tests/e2e/chat/virtualize-messages.spec.ts` — asserts: opening a LONG loaded conversation renders only a small subset of `[data-testid="chat-message"]` DOM nodes (mounted count « loaded count), and scrolling changes WHICH messages are mounted (a message near the top is unmounted after scrolling to the bottom, and vice-versa) — proving the list is virtualized while the scroll height is preserved.
- **TEST-4** (tier: e2e) [covers: ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/chat/lazy-load-jump-to-message.spec.ts` — asserts: the `#message-<id>` deep-link to an unloaded message still loads the around-window and centers + highlights the target — now via the virtualizer's `scrollToMessageId`/`scrollToIndex` (the target may be loaded-but-virtualized-out). Regression guard for the virtualized jump path.
- **TEST-5** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/lazy-load-messages.spec.ts` — asserts: under virtualization the prepend scroll-anchor invariant still holds — after older messages load on scroll-up, the viewport `scrollTop` grows by ~the prepended content height (no teleport). The objective check for the index-based anchor.
- **TEST-6** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/conversation-find.spec.ts` — asserts: server-side find of a match in an UNLOADED (and now virtualized-out) message still surfaces it in the results list and jumps to it (via `scrollToMessage` → `scrollToIndex`), centering + highlighting. Regression guard for the virtualized find path.

## Gate note

The diff touches `src-app/ui/**` only, so ≥1 `tier: e2e` test is required — satisfied
by TEST-3..TEST-6. The gallery/state-matrix coverage for ITEM-6 is enforced by
`npm run check` (the phase-8 `npm run check (ui): PASS` line), with TEST-3 also
exercising the virtual render at runtime.
