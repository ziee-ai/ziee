# FIX_ROUND-2

## Fix applied (the one new confirmed finding from round 2)

**FE — scope the interrupted signal to the displayed conversation (medium).**
`lastTurnInterrupted` is a single global store field, so it could leak across
conversations. Fixed on all three vectors:
- reset to `false` in the conversation-switch reset block (`Chat.store.ts` ~line 718);
- reset to `false` in the cache-hit restore (`loadConversationState`);
- gate the `complete`-handler write on `isOnOriginalConversation` so a BACKGROUND
  conversation completing cannot overwrite the on-screen flag.

The `error`-handler already scopes to the displayed conversation (its non-displayed
branch does not touch the flag); the transport-error catch runs only in the active
send path and is additionally covered by the switch reset.

Validation: UI `tsc --noEmit` clean; 6 frontend unit tests pass.

## Accepted limitation (not a fix — documented, low severity)

The empty-completion notice is derived at RENDER time from persisted message content
(the approved design; the client deliberately does not consume/persist
`finish_reason`). `interrupted` is transient client state, so after a **page reload**
(or once an interrupted turn is pushed off the last position by a later send), an
interrupted answerless turn will again render the notice. This is inherent to the
render-time design and is low severity — the copy reads as a reasonable generic
"no answer was produced" indicator in that state. A precise reload-robust fix would
require persisting the empty-vs-interrupted distinction (a per-message marker), which
the approved "no new persisted content type" decision excluded. Recorded in LEDGER
(round 2, MessageList.tsx) as `accepted-limitation`.

## Final convergence re-audit

A fresh blind SUBAGENT round could not be spawned — the account hit its monthly spend
limit mid-round-2 (2 of the 4 round-2 agents aborted for this reason). The final
convergence check was therefore a **direct self-review** of the full
`git diff origin/khoi...HEAD` across all audit angles (correctness, concurrency,
error-handling, state-management, regressions, api-contract, security, a11y,
patterns-conformance, tests-quality, perf, edge-cases, i18n), plus `tsc` + unit-test
re-runs. No new confirmed findings surfaced: the multi-iteration correctness path was
re-verified (round-2 agent J confirmed it reports NON-empty), the cross-conversation
bleed is fixed on every write/restore path, and the remaining item is the documented
render-time reload limitation above.

**New confirmed findings:** 0
