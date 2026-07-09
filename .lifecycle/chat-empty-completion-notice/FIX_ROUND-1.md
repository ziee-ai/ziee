# FIX_ROUND-1

## Fixes applied (from the round-1 blind audit — 6 agents, angles: correctness,
## error-handling, concurrency, api-contract, security, patterns-conformance,
## regressions, state-management, a11y, tests-quality, perf, i18n)

1. **BE — turn-wide visibility (medium).** Hoisted `turn_produced_visible_content`,
   OR-accumulated across agentic-loop iterations after each `finalize`, and used it
   in the terminal arm (was: per-iteration accumulator flag). Backend "empty" signal
   now matches the whole assistant message (frontend view).
2. **BE — extension-content classification (low).** The extension persist loop now
   extracts the block's actual `text` before `is_visible_answer` instead of hardcoding `""`.
3. **FE — interrupted-turn misattribution (medium).** Added a `lastTurnInterrupted`
   store flag (set on cancel/stream-error/abort, reset on send-start) threaded as an
   `interrupted` prop; the notice is suppressed for interrupted partials.
4. **FE — perf/pattern (low).** Wrapped the notice gate in `useMemo`.
5. **Tests.** Extracted `shouldShowEmptyCompletionNotice` + unit tests for the
   streaming/interrupted/user/answer gates (E1); added the fully-empty backend test
   (E3); corrected the e2e docstring (E4).
6. **Rejected (with rationale, see LEDGER):** the `_ => true` catch-all (deliberate
   safe default, matches the frontend); the a11y/i18n observations (no-defect).

Validation after fixes: `cargo check -p ziee --tests` clean (one pre-existing
unrelated dead-code warning); UI `tsc --noEmit` clean; 6 frontend unit tests pass.

## Re-audit (round 2 — blind, whole updated diff)

Spawned 4 blind agents (angles: correctness/concurrency/error-handling;
state-management/regressions/api-contract; tests-quality/patterns/a11y/i18n;
perf/edge-cases/security). **2 completed; 2 aborted on an account monthly-spend
limit** (correctness and tests agents) — their angles were covered by round-1
agents A/E and by the completed round-2 agents + a direct self-review.

New confirmed finding in round 2:
- **FE state-management (medium):** `lastTurnInterrupted` is a single global field —
  not reset on conversation switch / cache-restore, and written even for a background
  conversation's `complete` — so it leaks across conversations (wrongly suppresses or
  shows the notice on the displayed conversation). Confirmed independently by two
  agents and by first-hand reading.

**New confirmed findings:** 1
