# FIX_ROUND-2

## Fix applied (NEW-2 from round 1)

- **concurrency (low) — global `finalizingTurn` clobber across a switch**: the switched-away
  post-await branches used to `set({ finalizingTurn: false })`, which — because `finalizingTurn`
  is a single global flag — could clear a *different* conversation's in-progress suppression in a
  double-finalize-across-a-switch race. **Fix:** the switched-away branches now write nothing; the
  conversation switch itself (`loadConversation` cleanup, `reset`, snapshot restore) already resets
  `finalizingTurn`, so only the still-on-screen branches clear it.

## Final blind round (full diff)

Verified by a fresh blind auditor over the whole `chat` diff:
- Both original mediums (background-stream clobber, stuck-generating snapshot) **RESOLVED**.
- NEW-1 (afterStreamComplete-throw strands the flag) **NOT reachable** — `registry.afterStreamComplete`
  swallows every extension throw (registry.tsx:681-699).
- NEW-2 (global-flag clobber) **FIXED** (this round).
- `finalizingTurn` **cannot get stuck true** on any enumerated path (on-screen success /
  getHistory-error / no-conversation / switched-away / error-frame / send-catch), and the notice is
  suppressed with NO intermediate answerless render where `isStreaming===false && finalizing===false`.
- **ZERO new high/medium findings.**

## Residual (rejected — non-actionable, non-regressive)

- **NEW-3 (low)**: switch-away mid-finalize → cache-hit return could, for a
  *reasoning-only-streamed but persisted-has-text* turn, briefly flash the notice until the next
  sync refetch. Rejected: **not a regression** (the pre-fix code deleted the row, showing a MISSING
  assistant turn — strictly worse), self-heals on sync, and needs an unusual streamed-vs-persisted
  divergence. A robust fix (snapshotting the persisted tail on switch-away) is disproportionate to a
  self-healing cosmetic low. Documented in LEDGER + surfaced to the human in NOTES.

## Revert-check (TEST-3 integrity)

Ran `streaming-handoff-no-flicker.spec.ts` against the BASE `Chat.store.ts` (origin/khoi) with the
new spec present: it **FAILS** at line 136 — `expect(assistantBubble).toContainText('Hello from the
stream')` → "element(s) not found" (the base deletes the row during the handoff gap). With the fix
it **PASSES**. The spec genuinely catches the bug and cannot false-green.

**New confirmed findings:** 0
