# FIX_ROUND-1 — merge ledger, resolve, re-audit

## Ledger disposition
The Phase-6 blind audit surfaced **0 confirmed production defects** across 15 angles (correctness,
edge-cases, regressions, error-handling, security, dom-clobbering, redos, api-contract,
patterns-conformance, naming, a11y, perf, state-management, i18n, tests-quality). All confirmed
ledger rows were "no-issue" verifications.

Two **suspected** (not confirmed) low items were test-robustness questions about the e2e fixture,
now **resolved empirically** by running the e2e:
- **[resolved]** The 4-space-indented `> excerpt` DID attach inside footnote 1's `<li>` — TEST-5's
  `details.footnote-quote` "opens on click" assertion passed in the green runs.
- **[resolved]** `sup a` resolved to the forward footnote reference (the click opened the References
  section and the target resolved to an `LI` in the same bubble) — confirmed by TEST-5 green.

No production code required a change → no fix applied.

## Red/green reproduction (root-cause confirmation)
- **RED**: reverting `useStreamdownComponents.tsx` to the pre-fix (single-`user-content-` prefix) code
  and running TEST-5 against the real render path → after clicking the footnote reference,
  `details.footnote-section` stayed `open: false` (spec line 296) — the click no-ops. This reproduces
  the exact reported symptom and confirms the double-prefix id mismatch is the cause.
- **GREEN**: with the fix, TEST-5 (`clicking a footnote reference expands References + cited excerpt and
  resolves the target`) and TEST-6 (`footnote reference click is scoped per message`) both PASS.

## Re-audit
Re-reviewed the full diff after confirming the above; no new confirmed findings surfaced.

**New confirmed findings:** 0
