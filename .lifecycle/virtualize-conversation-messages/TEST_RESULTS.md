# TEST_RESULTS — virtualize-conversation-messages

Frontend-only diff (`src-app/ui/**`; no backend, no `desktop/ui`). All phase-3
tests authored + run green.

## Frontend static gate

- npm run check (ui): PASS

## Unit

- **TEST-1**: PASS — `messageWindow.test.ts` `indexOfMessageId` (array index or -1)
- **TEST-2**: PASS — `scrollAnchor.utils.test.ts` `indexRestoreOffset` (re-pin offset + clamp)

(`node --test` → 14 pass / 0 fail across the two files.)

## E2E

- **TEST-3**: PASS — `tests/e2e/chat/virtualize-messages.spec.ts` (long conversation mounts only a subset of `chat-message` DOM nodes; scrolling re-windows — oldest mounts / newest unmounts)
- **TEST-4**: PASS — `tests/e2e/chat/lazy-load-jump-to-message.spec.ts` (`#message-<id>` deep-link to an unloaded message centers + highlights it via the virtualizer's `scrollToMessageId`; scroll-down pages newer)
- **TEST-5**: PASS — `tests/e2e/chat/lazy-load-messages.spec.ts` (prepend scroll-anchor invariant holds under virtualization: scrollTop grows by ~the prepended height, no teleport; stable across repeated runs)
- **TEST-6**: PASS — `tests/e2e/chat/conversation-find.spec.ts` (server-side find of a match in an unloaded/virtualized-out message jumps to it via `scrollToMessage`→`scrollToIndex`, centering + highlighting)

Regression (lazy-load specs re-run under the virtualized code, all green):
`lazy-load-branch-reset` + `lazy-load-sse-and-short` also PASS.

(Run: `npx playwright test <6 specs> --workers=1` → 7 passed [TEST-12/sse is 2
cases]; the anchor spec additionally verified stable across `--repeat-each=3`.)
