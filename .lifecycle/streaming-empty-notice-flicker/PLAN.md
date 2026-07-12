# PLAN — fix: mid-stream flicker + false "empty response" notice

## Summary

While the assistant streams, its answer disappears for a beat, the warning
**"The model returned an empty response and made no tool call."** flashes, then the answer
reappears. Root cause: the on-screen `complete` SSE handler in
`src-app/ui/src/modules/chat/core/stores/Chat.store.ts` (~L1527) removes the streaming assistant
row from the `messages` Map **and** flips `isStreaming:false` in one `set()`, then re-merges the
persisted row only **after** an awaited `getHistory` (`reconcileTail`/`loadMessages`). In that gap
the last assistant slot is empty/absent while `isStreaming===false`, so the render-time
empty-completion notice (`ChatMessage.tsx` → `emptyCompletion.ts::shouldShowEmptyCompletionNotice`)
fires and the content visibly flickers. Fix = make the streaming→persisted handoff **atomic** (one
`set()`), plus a defensive notice gate.

## Items

- **ITEM-1**: Make the on-screen (`isOnOriginalConversation`) `complete` handler swap
  streaming→persisted **atomically in a single `set()`** — keep the streamed assistant row visible,
  fetch the persisted tail, then in ONE update drop the streaming placeholder + merge the persisted
  tail (or snap-to-tail on the branch-changed/`hasMoreAfter` path) + clear
  `isStreaming/sending/streamingMessage/streamingMessageId/streamingAbortController/branchChangedDuringStream`
  + set `lastTurnInterrupted` for the cancelled case. No empty/absent frame; `isStreaming` never
  false while the row is empty.
- **ITEM-2**: Add pure helper `finalizeTailWindow(existing, tailPage, streamingId)` in
  `messageWindow.ts` — delete `streamingId` from a copy of the window, then `mergeTailWindow` the
  persisted tail (real-id → re-added in place at tail; synthetic placeholder-id → dropped, persisted
  appended). One assistant row for both id cases; never empty.
- **ITEM-3**: Add transient store flag `finalizingTurn: boolean` (state interface + initial
  `false`); set `true` when the on-screen `complete` finalization begins, cleared in the atomic
  `set()` (and every terminal fallback of that path). Thread it through `MessageList.tsx` (gated to
  the last assistant row, like `isStreaming`/`lastTurnInterrupted`) into `ChatMessage.tsx` as a
  `finalizing` prop.
- **ITEM-4**: Add `!finalizing` conjunct to `shouldShowEmptyCompletionNotice` (extend its opts
  signature) so the notice can never render while a post-stream reconcile is in flight — defensive
  insurance even if a future path reintroduces a gap.
- **ITEM-5**: Preserve the genuinely-empty COMPLETED-turn behavior and reload-robustness: a real
  empty turn still shows the notice once, stable, and after reload (existing
  `empty-completion.spec.ts` stays green; `finalizing` is false by the time the turn is finalized).
- **ITEM-6**: Verify the #135 approval-scroll (`ConversationPage.tsx:318-363`) still fires once when
  a pending approval appears below the fold now that the flicker/remount is gone; add a minimal
  re-assert guard ONLY if a residual re-measure race is observed.
- **ITEM-7**: Keep the error / user-cancel / background-conversation / reset null-sites correct —
  cancel still shows the partial (no false empty notice), a background conversation completing does
  not overwrite the on-screen `lastTurnInterrupted`, and the getHistory-failed fallback keeps the
  streamed row visible.

## Files to touch

- `src-app/ui/src/modules/chat/core/stores/Chat.store.ts` — atomic `complete` handler (~1512-1577);
  add `finalizingTurn` to `ChatState` + initial state.
- `src-app/ui/src/modules/chat/core/stores/messageWindow.ts` — new `finalizeTailWindow` helper.
- `src-app/ui/src/modules/chat/core/stores/messageWindow.test.ts` — extend for `finalizeTailWindow`.
- `src-app/ui/src/modules/chat/components/emptyCompletion.ts` — `!finalizing` conjunct + opts.
- `src-app/ui/src/modules/chat/components/emptyCompletion.test.ts` — extend the gate matrix.
- `src-app/ui/src/modules/chat/components/MessageList.tsx` — read `finalizingTurn`, thread to last
  assistant row (~96-101, 545-576).
- `src-app/ui/src/modules/chat/components/ChatMessage.tsx` — accept `finalizing` prop, pass to gate
  (~20-73).
- `src-app/ui/src/modules/chat/pages/ConversationPage.tsx` — approval-scroll re-assert guard
  (CONTINGENT on ITEM-6 repro).
- `src-app/ui/tests/e2e/chat/streaming-handoff-no-flicker.spec.ts` — new e2e (flicker + approval).

## Patterns to follow

- **Pure window helper + node:test**: mirror `mergeTailWindow`/`appendWindow` in `messageWindow.ts`
  and its existing `messageWindow.test.ts` — same signature style, same doc-comment + unit-test idiom.
- **Gate predicate + matrix test**: mirror `emptyCompletion.ts` + `emptyCompletion.test.ts` — add
  the conjunct to the pure predicate and a row to the truth-table test; do not rewrite.
- **Prop threading to the last assistant row**: mirror how `MessageList.tsx` already derives and
  passes `isStreaming`/`interrupted` (both the virtualized 545-557 and plain 567-576 paths).
- **E2E SSE mock**: mirror `tests/e2e/chat/empty-completion.spec.ts` using
  `tests/e2e/helpers/sse-mock-helpers.ts` (`mockChatStream`, `startedEvent`, `completeEvent`,
  `mockGetMessages`) — assert the DOM EFFECT (text present / notice absent during the widened gap),
  not a spy.

## UI-surface checklist

This diff adds **no new page/drawer/card/panel** — it fixes render behavior of the existing
`ChatMessage` empty-completion notice inside the existing `MessageList`/`ConversationPage`. So:
- **Precedent** — n/a (no new surface). The touched notice already mirrors the `Alert` kit usage;
  no visual change to it.
- **Scale / cardinality** — n/a; the message window is already virtualized + lazy-paged
  (`MESSAGE_PAGE_SIZE=30`); this change does not alter paging, only the finalize timing.
- **Device size / responsive** — the fix touches both the virtualized (desktop) and plain (mobile)
  render paths in `MessageList`; the e2e/gate run covers the desktop path, and the mobile path uses
  the identical `finalizing` prop. No layout change → no new narrow-viewport gallery state needed.
- **User-visible progress** — the streaming "generating" affordance now clears when the finalized
  turn is on screen (one fast getHistory later) instead of a beat early; this REMOVES a flicker, it
  adds no new spinner.

## Iteration 2 — resume-chain flicker (human review found the tool-approval case)

- **ITEM-8**: Fix the multi-tool / tool-approval RESUME disappear. On a resume the streaming
  content handler (`Chat.store.ts` `applyStreamFrame`, content path) created a placeholder with
  `contents: []` keyed by `data.message_id`, which OVERWRITES the existing assistant row (the
  resume continues the SAME message id) → the row renders empty → `ChatMessage` bails to `null`
  (zero blocks) → the bubble VANISHES then reappears. Fix: reuse the existing assistant row as the
  streaming buffer when one exists for that id (new frames append), via the pure helper
  `resumeOrFreshPlaceholder`. Mirror the `finalizeTailWindow` helper pattern in `messageWindow.ts`.

### Iteration-2 context
- Branch updated onto current `origin/khoi` (merge), which now includes `#137` (tool_use/tool_result
  pairing) and `#138` (stale-artifact links). On the merged backend the empty-completion NOTICE in
  the multi-tool flow is already resolved by those; the remaining frontend defect was the resume
  disappear (ITEM-8). Verified live against a merged-code backend + real gpt-oss over multiple
  approve-and-fetch turns: no disappear, no notice.
