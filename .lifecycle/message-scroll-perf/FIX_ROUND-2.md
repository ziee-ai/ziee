# message-scroll-perf — FIX_ROUND-2

## Fixes applied to the FIX_ROUND-1 (round-1) findings

The 8 new confirmed findings from the round-1 re-audit were fixed:

- **MEDIUM perf + LOW perf** (`MessageList.tsx`) — `initialMeasurementsCache` is now
  built ONCE and FROZEN in a `seedRef` after the first non-empty build (virtual-core
  consumes the seed only while its `measurementsCache` is empty). Streaming no
  longer rebuilds the seed O(window)/token nor mutates the LRU during render.
- **LOW security** (`ReservedImage.tsx`) — added `onError` → `setLoaded(true)`, so a
  broken/404 dimensionless image releases its 240px reservation (no phantom gap).
- **MEDIUM tests** (`message-scroll-perf.spec.ts`) — warm-start assertion tightened
  to `warmSH/finalSH ∈ (0.95, 1.05)` (a working seed restores the exact measured
  total ~1.0; the estimator would be materially off), isolating the cache path.
- **LOW tests** (`message-scroll-perf.spec.ts`) — `settledScrollHeight` now requires
  3 consecutive equal reads (budget 40×120ms), guarding against a transient
  Shiki-highlight plateau.
- **LOW correctness / LOW patterns** (`MessageList.tsx`, `estimateMessageHeight.ts`) —
  the mid-session resize-reseed intent is documented honestly (virtual-core can't
  re-consume the seed mid-mount; the visible rows re-measure at the new width
  anyway); the estimator default width aligned to MessageList's 864 fallback.
- **LOW correctness** (`MessageList.tsx`, count>0-at-mount) — documented as latent
  (the store clears messages on switch, so a real mount starts at count 0).

## Blind verify round (round 2) — findings

A verify blind round (correctness/perf/concurrency + security/tests/state) on the
frozen-seed diff:

- The security/tests/state-management pass returned **NO_FINDINGS** — the onError
  fix, the tightened warm-start band (verified ~1.0 achievable, no flake at the
  1280px→864 test viewport), the strengthened poller, and the default-width change
  (7/7 unit tests pass at 864) were all confirmed sound; no security regression.
- The correctness/perf pass found ONE confirmed defect: the seed FREEZE regressed
  the warm-start on the **dominant navigation path**. Conversation switching is
  IN-PLACE (route keyed by the literal `/chat/:conversationId`, MessageList not
  unmounted); the store clears messages (count N_A→0→N_B). A `seedRef` frozen to
  conversation A's seed was reused for B (B's rows miss A's UUID keys → fall back
  to estimate → B loses its persisted warm-start; no wrong heights, but the
  ITEM-2 benefit was lost after the first conversation per page load).

## Fix

`MessageList.tsx` — the seed memo now resets `seedRef.current = null` when
`messagesArray.length === 0`. A conversation switch passes through count 0 (store
clear) → the freeze drops → B builds and freezes its OWN seed at the 0→N_B
transition, which virtual-core re-consumes (its `measurementsCache` is empty at
that point). Streaming never reaches count 0, so it never resets → the anti-churn
freeze still holds through a stream. Committed as "reset seed freeze on window
empty (conversation switch re-seeds)".

**New confirmed findings:** 1
