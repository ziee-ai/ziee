# DRIFT-2 — reconciliation after the Phase-6 blind audit

The blind audit surfaced two confirmed defects that change the implementation; both
are fixed here and the plan artifacts amended.

- **DRIFT-2.1** — verdict: plan-wins — TESTS.md/ITEM-6 claim a "≥40px tap target", but the
  toggle rendered `Button size="icon"` = `size-8` (32×32px), so TEST-6 would FAIL and the mobile
  tap-target guideline was missed (confirmed by two audit angles + the gallery G5 geometry data).
  Fix: `AuthThemeToggle` now sets `className="… size-11"` (44px, overrides the kit icon size via
  `cn`/tailwind-merge). Impl re-aligned to the plan; the responsive test now measures a real
  ≥40px target.
- **DRIFT-2.2** — verdict: impl-wins (DEC-6 amended) — the per-host dynamic `data-testid={prop}`
  scheme made both toggle ids invisible to the regex testid-registry generator (they dropped out
  of the typed registry). Fix: one shared static literal `data-testid="auth-theme-toggle"` on the
  toggle; `themeToggleTestId` prop removed from `AuthScreenLayout`; `AuthPage`/`SetupPage` pass
  nothing; the setup + responsive specs select `auth-theme-toggle`. DEC-6 updated. `app-setup-
  theme-toggle` retired (no origin/main test referenced it — verified).

Findings triaged but NOT fixed (recorded with rationale in LEDGER.jsonl, re-audited clean):
- header-placement divergence (login Title above card vs setup Title inside card) — LOW,
  pre-existing content difference, out of the theming scope; the SHARED CHROME (backdrop/toggle/
  landmark/container) is what makes the two pages twins. Not a regression.
- meta-color poll waits for existence not value — LOW, safe today (index.html ships no static
  meta[theme-color], so the tag is created by the first rAF); documented.
- hardcoded English aria-label — LOW, ported verbatim from SetupThemeSwitcher; the whole app is
  English-hardcoded (no i18n layer). No regression.

**Unresolved drifts:** 0
