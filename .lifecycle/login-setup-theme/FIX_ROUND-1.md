# FIX_ROUND-1 — fix + re-audit

## Fixes applied (from the Phase-6 ledger)
1. **HIGH (responsive/test-reality):** toggle tap target was `Button size="icon"` = 32×32px while
   TEST-6 asserts ≥40px and the mobile guideline wants larger. Fixed: `AuthThemeToggle` className
   `size-11` (44px) — `cn`=`twMerge(clsx())` makes `size-11` win over the variant's `size-8`
   (independently re-verified). Real ≥40px target; TEST-6 now meaningful.
2. **LOW (affordance-parity/testid):** dynamic `data-testid={prop}` made both toggle ids invisible
   to the regex testid-registry generator. Fixed: one static `data-testid="auth-theme-toggle"`;
   prop plumbing removed; specs + registry updated; `auth-theme-toggle` present in
   `testIds.generated.ts`.
3. **Incidental:** `right-4` → `end-4` (logical-direction lint, RTL-safe) surfaced by the check.

Findings triaged as rejected/not-a-regression (with rationale in LEDGER.jsonl): header-placement
divergence (pre-existing, out of scope), meta-color existence-poll (safe: no static meta tag),
hardcoded English aria-label (ported; app has no i18n layer).

## Re-audit (fresh blind agent, diff-only)
A fresh blind reviewer re-audited the full diff focusing on the two fixes + a general re-scan
(correctness, a11y single-main, state-management, tests-quality/test-reality, responsive). It
independently confirmed:
- `size-11` wins tailwind-merge over `size-8` → real 44px tap target.
- one static `auth-theme-toggle` in `testIds.generated.ts`; never two toggles on one page.
- the setup card (~640px) centered in `min-h-dvh` clears the 44px toggle band at 390×844 → the
  no-intersection assertion is TRUE.
- exactly one `<main>` landmark per page; `end-4` correct; tests exercise real behavior.

Result: no new defects, no regressions from the fixes.

**New confirmed findings:** 0
