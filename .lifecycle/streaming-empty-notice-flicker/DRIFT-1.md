# DRIFT-1 — implementation vs plan

Reconciling the shipped code against PLAN.md ITEM-1..7.

- **DRIFT-1.1** — verdict: none — ITEM-1 atomic `complete` handler implemented as planned: the
  on-screen path keeps the streamed row, fetches persisted tail, then drops the placeholder + merges
  + clears all streaming flags in ONE `set()`. Background path split into an early return, byte-for
  behavior identical to the original background teardown.
- **DRIFT-1.2** — verdict: none — ITEM-2 `finalizeTailWindow` added next to `mergeTailWindow`, pure,
  unit-tested (TEST-1). Delete-then-merge collapses both the synthetic-id and real-id cases to one
  assistant row.
- **DRIFT-1.3** — verdict: impl-wins — ITEM-3 `finalizingTurn` was additionally reset at the
  conversation-switch cleanup, snapshot-restore, `sendMessage` start, and `reset()` sites (not only
  cleared in the atomic set). PLAN listed only "set true on finalize begin / cleared in the atomic
  set"; the extra resets prevent the flag from ever latching true across a switch/reset. PLAN.md
  Files list already names `Chat.store.ts`; no plan-item scope change, so PLAN amendment not
  required (behavior strictly safer). Recorded here as the rationale.
- **DRIFT-1.4** — verdict: none — ITEM-4 `!finalizing` conjunct added to
  `shouldShowEmptyCompletionNotice` + threaded via `MessageList` → `ChatMessage` exactly as planned.
- **DRIFT-1.5** — verdict: none — ITEM-5 preserved: `emptyCompletion.test.ts` asserts a genuinely
  empty turn (finalizing=false) still shows the notice; existing `empty-completion.spec.ts` (TEST-4)
  unchanged.
- **DRIFT-1.6** — verdict: resolved — ITEM-6 approval-scroll: PLAN flagged a possible
  `ConversationPage.tsx` guard as CONTINGENT. No change made (the atomic handoff removes the
  remount, and the existing #135 spec `07-mcp/tool-group-single-artifact.spec.ts` is the regression
  guard, now TEST-5). The contingent guard remains deferred to Phase-8/live observation; if a
  residual race appears there it will be a new drift round. Not an unresolved divergence now.
- **DRIFT-1.7** — verdict: none — ITEM-7 error/cancel/background/reset null-sites left unchanged
  except the on-screen `complete` path; the getHistory-failed fallback keeps the streamed row and
  clears the flags (no hang).

**Unresolved drifts:** 0
