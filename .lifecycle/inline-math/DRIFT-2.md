# DRIFT-2 — implementation vs plan, after the phase-7 fix round

- **DRIFT-2.1** — verdict: impl-wins — the phase-7 re-audit found a corruption case no
  planned guard covered: two directly adjacent pairs, `\( a \)\( b \)`, emit `$a$$b$`,
  and because a math-text closer must be a `$` run of the same length as its opener, the
  inner `$$` does not close the first span — the run collapses into ONE span whose body is
  `a$$b`, which KaTeX rejects. ITEM-4's paragraph guard structurally cannot catch it: that
  guard looks for a `$` already present in the SOURCE, while this collision is
  manufactured by the rewrite. Added as **ITEM-9** with **TEST-20**; PLAN.md and TESTS.md
  amended and phases 1–3 re-run green.

- **DRIFT-2.2** — verdict: resolved — the first implementation of ITEM-9 used
  `str.slice(0, offset).endsWith('\\)')`, which copies the entire prefix on every match
  and would have reintroduced exactly the quadratic per-match cost that the phase-6 fix
  (hoisting `hasLiveDollar`) had just removed. Rewritten to two indexed character reads.
  Caught by re-reading the round-1 fix rather than by a test — a passing suite would never
  have surfaced it.

- **DRIFT-2.3** — verdict: none — the `normalizeInlineMath` docblock's guard count was
  updated from six to seven to match ITEM-9. Documentation accuracy only.

**Unresolved drifts:** 0
