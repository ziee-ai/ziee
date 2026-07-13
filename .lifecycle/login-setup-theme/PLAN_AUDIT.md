# PLAN_AUDIT.md — login-setup-theme (audited against the codebase)

## Breakage risk
- `AuthPage` dropping `BlankLayoutComponent`: BlankLayout provides the `main` landmark +
  `useMetaThemeColor` + root-bg flash guard. `AuthScreenLayout` folds ALL of these in, so no
  regression. Verified BlankLayout's only responsibilities are those three (read
  `modules/layouts/blank/BlankLayout.tsx`). Its other consumer(s) (e.g. `AuthCallbackPage`,
  `LinkAccountPage`) are NOT touched and keep using `BlankLayoutComponent` — the component
  is not removed. LOW risk.
- `SetupPage` deleting `SetupBackdrop`/`SetupThemeSwitcher`: both are file-local functions
  with no external importer (grep-confirmed page-local). Replacing with `AuthScreenLayout`
  preserves the `app-setup-*` testids used by the setup e2e suite (16 specs) — the toggle
  testid `app-setup-theme-toggle` is preserved via the shared toggle's `data-testid` prop, and
  `app-setup-card`/`-form`/inputs stay inside the card. LOW risk, but the full setup e2e suite
  is the regression backstop (Phase 8).
- `git mv setup-clouds.webp → auth-clouds.webp`: only `SetupPage.tsx` imports it today; after
  the refactor only `AuthScreenLayout` imports it. Confirmed no other importer. LOW risk.
- Adding `--auth-backdrop` to `index.css`: append-only token block; existing tokens untouched.
  Must regen `DESIGN_SYSTEM.md` or `check:design-spec` fails — captured as a Phase-8 gate line.
- Removing the `data-allow-custom-color` raw-hex escape hatch in favor of a token: the
  `lint:colors` gate PASSES a `var(--token)` reference, so this is strictly cleaner. LOW risk.

## Pattern conformance
- Toggle mirrors `SetupThemeSwitcher` + `ThemeSettings` (same `useTheme`/`ConfigClient` path) —
  conforms to the "reuse the existing theme mechanism" rule (human ask #1). PASS.
- Backdrop mirrors `SetupBackdrop` structure — PASS.
- `AuthScreenLayout` mirrors `BlankLayout`'s landmark + meta-color + flash-guard idioms — PASS.
- Token addition + `gen:design-spec` regen mirrors the DESIGN_SYSTEM.md generation contract — PASS.
- Gallery coverage entries mirror existing `'via'`/`'flow'` entries — PASS.

## Migration collisions
- None. No migration added (frontend-only). `ls migrations | tail -1` = `...157`; unaffected.

## OpenAPI regen
- None. No request/response type changes. Neither `ui/` nor `desktop/ui/` client regenerates.
  (Therefore the Phase-3/Phase-8 frontend gates apply because of the `.tsx`/`.css` edits, NOT
  because of any generated-file change.)

## Per-item verdicts
- **ITEM-1** — verdict: PASS — mirrors `SetupThemeSwitcher`; reuses `useTheme`; no new mechanism.
- **ITEM-2** — verdict: PASS — folds `BlankLayout` semantics + `SetupBackdrop` into one shared
  component; no new subsystem, only composition of existing idioms.
- **ITEM-3** — verdict: PASS — `AuthPage` swaps scaffolding for `AuthScreenLayout`; preserves all
  `auth-*` testids consumed by `tests/e2e/auth/*`.
- **ITEM-4** — verdict: CONCERN — `SetupPage` is covered by 16 existing e2e specs; refactor MUST
  preserve every `app-setup-*` testid and the redirect/validation behavior. Mitigation: keep the
  `Card`/`Form` subtree byte-for-byte, change only the outer wrapper; run the full setup suite in
  Phase 8. Not BLOCKED (no plan change needed), just flagged for careful diffing.
- **ITEM-5** — verdict: CONCERN — requires `gen:design-spec` regen or `check:design-spec` (inside
  `npm run check`) fails. Also the `--auth-backdrop` dark value must be chosen so AA contrast of the
  card/text over it holds (verified by axe in TEST-5). Resolved in DECISIONS DEC-4/DEC-5; budgeted
  as a Phase-8 gate line. Not BLOCKED.
- **ITEM-6** — verdict: PASS — responsive is CSS-only (`max-w-md`, `p-4`, full-bleed backdrop);
  enforced by a 390px gallery state + TEST-6.

No `BLOCKED` verdicts.
