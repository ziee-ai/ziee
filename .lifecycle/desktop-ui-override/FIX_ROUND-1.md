# FIX_ROUND-1

Two blind auditors (diff-only) reviewed `git diff main...HEAD` across correctness,
security, error-handling, patterns-conformance, api-contract, regex-robustness,
state-management, desktop/web-parity, scope-drift, a11y, and the Drawer guard.

Both concluded the mechanism is sound and desktop/web parity holds at every real
use site. Confirmed findings and their fixes:

- **[MED] Drawer stacking guard keyed off the overridable `data-testid`** →
  fixed: query the stable `[data-slot="layout-drawer"]` marker (the one added for
  exactly this purpose), so a caller-overridden testid can't make a stacked drawer
  invisible to the guard.
- **[MED] Desktop auth barrel was an incomplete shadow** (missing `useAuthStore`
  vs core) with a false rationale → fixed: made byte-complete with core's barrel +
  corrected the comment (defensive shadow; the live path uses the `@/…/AuthGuard`
  form directly).
- **[MED] Manifest `UIOverrides` extraction truncated multi-line object seam
  values + matched nested keys** → fixed: brace-balanced depth-0 extractor
  (`topLevelSeamKeys`) + regression test (TEST-7, now 5 cases).
- **[LOW] Stale doc ref to `overrides.tsx`** → fixed to `overrides/hardware-monitor.tsx`.

Rejected (with rationale in LEDGER.jsonl): the strict-`z` equal-z guard case
(matches core's contract — stacked drawers pass an elevated zIndex), the
pre-existing build-time resolver traversal (dev-controlled, no new surface, no
web leak), and three advisory/codemod/lint-corner LOWs.

All fixes re-verified: desktop `tsc` clean, Drawer + manifest tests green.

**New confirmed findings:** 4
