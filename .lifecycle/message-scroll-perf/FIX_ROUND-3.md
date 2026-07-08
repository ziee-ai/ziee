# message-scroll-perf — FIX_ROUND-3 (convergence)

## Change under review

The single fix from FIX_ROUND-2: `MessageList.tsx` resets `seedRef.current = null`
when `messagesArray.length === 0`, so an in-place conversation switch (count
N_A→0→N_B) drops the previous conversation's frozen seed and lets the new
conversation build + freeze its own — restoring the warm-start on every switch
while keeping the anti-churn freeze during streaming (which never reaches count 0).

## Final blind round

A fresh blind auditor traced the delta against `@tanstack/virtual-core`'s seed
consumption (`getMeasurements` re-reads `initialMeasurementsCache` whenever its
`measurementsCache.length === 0`, which the count-0 render produces) and verified:

1. Conversation B re-warm-starts (fresh `seedRef` → B's ids built + consumed at
   0→N_B).
2. Streaming stays frozen (count never hits 0 → no per-token rebuild, no LRU
   churn, stable option identity).
3. The ref mutation in `useMemo` is the accepted render-cache pattern, idempotent
   and StrictMode-safe; `EMPTY_SEED` is never stored into the ref, so no
   wrong/empty freeze; no length oscillation → no infinite rebuild.
4. Strictly better than the prior behavior; in the worst edge it is a *missed*
   warm-start (falls back to the estimator), never a wrong-conversation seed.

Verdict: **NO_FINDINGS**.

**New confirmed findings:** 0
