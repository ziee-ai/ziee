# FIX_ROUND-1 — merge the ledger, fix, re-audit

## Audit-conduct note (deviation, recorded rather than hidden)

The skill prescribes spawning fresh/blind subagents for the phase-6 audit. This session
operates under a standing instruction not to invoke the Agent tool, so the multi-angle
audit was conducted **directly by the implementing agent** across the 16 angles recorded in
`LEDGER.jsonl`. That is a genuine weakening of the "blind" property — the auditor shares
the implementer's assumptions — and it is stated here plainly rather than left implied.
Two mitigations were applied: every angle was worked from the raw
`git diff origin/main...HEAD --unified=0` hunk list rather than from memory of the design,
and every behavioral claim was settled by running code (the unit suite, the stash-and-rerun
baseline comparisons) rather than by reading it (B7).

## Findings fixed this round

| Ledger angle | Severity | Fix |
|---|---|---|
| correctness (`markdownPreprocess.ts:75`) | high | Widened the early return to admit `\\(` (ITEM-8). Without it the feature was a no-op for any message lacking a `[`. |
| tests-quality (`markdownPreprocess.test.ts:146`) | high | Rewrote the test that asserted the no-op was *correct*; added TEST-13 asserting through `preprocessMarkdown`, the only level at which the caller's guard is observable. |
| correctness (unpaired-`$` hijack) | medium | `paragraphHasLiveDollar` guard (ITEM-4), paragraph-scoped and escape-aware, "any live `$`" rather than odd-count. |
| perf (`normalizeMathDelimiters.ts:268`) | medium | Hoisted a single O(n) `hasLiveDollar(md)` out of the per-match path; the paragraph scan now runs only when the string contains a live `$` at all. Exactly equivalent, not an approximation. |
| test-reality (`markdown-rendering.spec.ts:355`) | medium | Strengthened the prose e2e from `.katex > 0` to an exact count of 2 plus a negative assertion that `s/(foo)/bar/` — what the DOM showed before this feature — is gone. |
| correctness (part-local guards) | low | Documented the two inherited part-local limits (paragraph guard and `lineHead` blind across an inline-code split) in the `normalizeInlineMath` docblock instead of silently leaving them. |
| maintainability (BRE guard) | low | Documented that `\\{`/`\\}`/`\\|` also skip genuine set-brace and norm notation — a deliberate cost of the user-chosen guard, degrading to today's rendering. |

Findings recorded and deliberately **not** actioned, with rationale in the ledger:
security (KaTeX `trust` verified default-false at `streamdownPlugins.ts:35`), error-handling,
api-contract, patterns-conformance, modularity, extensibility, a11y, i18n-copy,
state-management, api-friendliness — each inspected and found to carry no defect.

## Re-audit (full second pass over the fixed diff)

**New confirmed findings:** 1

- **correctness / high** — `\( a \)\( b \)`: two adjacent pairs emit `$a$$b$`, which
  collapses into a single span with the body `a$$b` because a math-text closer must match
  its opener's run length. Not reachable by any existing guard. Fixed as **ITEM-9** with
  **TEST-20**, and its first implementation's O(n)-per-match prefix slice was itself caught
  and rewritten to indexed reads (DRIFT-2.2).
