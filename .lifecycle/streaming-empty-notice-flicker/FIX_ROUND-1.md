# FIX_ROUND-1

Merged the phase-6 ledger (4 blind angles) → fixed every confirmed finding → re-ran a
blind round on the fix.

## Fixes applied

- **concurrency (medium, Chat.store.ts) — background-stream clobber**: the switched-away
  branch used to `set(clearStreaming)` unconditionally, nulling `streamingMessage`/`isStreaming`
  onto whatever conversation was now open. **Fix:** clear the streaming CONTROL flags
  SYNCHRONOUSLY (as the original did) in the on-screen path's first `set()`, keep the streamed
  row, and set `finalizingTurn:true`; the switched-away branch no longer writes those globals.
- **state-management (medium) — stuck "generating" snapshot**: `isStreaming` was held true across
  `await getHistory`, so a switch mid-await snapshotted the finished conversation as still
  streaming. **Fix:** same synchronous clear — `saveConversationState` now snapshots
  `isStreaming:false` during the finalize await.
- **error-handling (low) — lastTurnInterrupted cross-write**: now written only synchronously while
  on-screen; post-await branches never write it.
- **state-management (low) — finalizingTurn reset gap**: added `finalizingTurn:false` to the
  error-frame handler (3 sites) and the `sendMessage` catch.
- **perf (low) — spinner held across getHistory**: resolved as a side effect of the synchronous
  clear — the "generating" affordance clears immediately on `complete` again.

## Rejected (not defects)

- **perf (low) — double Map copy in `finalizeTailWindow`**: one redundant O(window) copy per turn,
  bounded by `MESSAGE_PAGE_SIZE`; keeping the `mergeTailWindow` reuse is worth more than the
  micro-opt. Rejected in LEDGER.

## Re-audit result (blind, fix-only diff)

- Both prior mediums **RESOLVED** (verified: control flags cleared synchronously before the await;
  switched-away branch writes only the transient flag).
- **NEW-1 (rejected)** — "afterStreamComplete awaited outside try could strand finalizingTurn":
  NOT reachable — `registry.afterStreamComplete` (registry.tsx:681-699) wraps every extension +
  its actions in try/catch and never throws.
- **NEW-2 (confirmed, low)** — `finalizingTurn` is a single global flag; a double-finalize across a
  conversation switch could let A's stale getHistory clear B's in-progress suppression. Carried to
  FIX_ROUND-2.

**New confirmed findings:** 1
