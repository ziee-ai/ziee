# FIX_ROUND-2 — convergence audit after round-1 fix (`param_policy` precedence)

A final blind reviewer traced the full row-override × thinking × reasoning ×
gpt-5 truth table (10 cases) against the revised `resolve()`:

- Every case matches the intended contract: the row override beats a CAPABILITY
  guess (family pattern / catalog / opus-restriction) but NOT the per-call
  thinking-active reconciliation; gpt-5 org-verification stays a hard constraint.
- `row_allows_sampling` computed once and used consistently; no dead code / no
  leftover from the removed end-block; the guard boolean has no off-by-one.
- Tests cover the thinking×row cases + row-beats-family + row-disable.

The reviewer noted a pre-existing edge (a catalog-`supports_sampling_params:true`
model that also matches a reasoning family is still re-dropped by the reasoning
guard) but confirmed it is unchanged by this revision and outside the truth
table — not a new bug.

**New confirmed findings:** 0
