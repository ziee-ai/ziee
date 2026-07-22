# FIX_ROUND-2 — full blind re-audit after the round-1 fixes

A complete second pass over the fixed diff across the same 16 angles, worked from the
regenerated `git diff origin/main...HEAD --unified=0` hunk list. The round-1 conduct note
(self-audited rather than subagent-blind, per a standing session instruction) applies here
too and is not restated.

## What this round specifically re-checked

- **The round-1 fixes themselves**, on the principle that a fix is the most likely place
  to introduce the next defect. This is what caught DRIFT-2.2 in round 1 (the adjacency
  guard's `slice(0, offset)` reintroducing quadratic cost); re-verified that the
  replacement is two indexed reads with an `offset >= 2` bound and cannot underflow.
- **Regex statefulness**, the classic latent bug in this shape of code. `INLINE_MATH_RE`
  carries `/g` but is used only via `String.replace`, which resets `lastIndex` itself;
  `BRE_SIGNAL_RE` is used with `.test()` and deliberately carries **no** `/g`, so it has no
  `lastIndex` to carry between calls. Both are module-level singletons shared across every
  render, so a stray `/g` on the `.test()` regex would have produced alternating
  pass/fail per call — checked explicitly, not assumed.
- **Guard interaction and ordering.** Confirmed each of the seven guards is independent
  and order-insensitive for CORRECTNESS (every branch returns the original text), so the
  cheapest-first ordering is purely a performance choice. Confirmed the adjacency guard
  cannot mask the paragraph guard or vice versa: a display-emitted `$$` still forces
  `anyDollar` true, which is what keeps TEST-11 green.
- **Idempotence under the new guard.** `\(a\)\(b\)` is skipped, so a second application
  sees the identical input and skips again — a fixed point. Confirmed by TEST-12, whose
  `ALL_INPUTS` replay automatically picked up the four new TEST-20 inputs.
- **The full unit suite and `tsc --noEmit`**, both clean (27/27 tests; no type errors).

## Result

**New confirmed findings:** 0

No new defect surfaced. The one open item leaving this phase is not a finding but a
recorded pre-existing condition of the branch base (three stale generated registries in the
`sdk` submodule and two unrelated failing store tests) — see DRIFT-1.5 and TEST_RESULTS.md.
