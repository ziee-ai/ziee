# DRIFT-1 — implementation vs plan

Reconciling the shipped code against PLAN.md / DECISIONS.md after the first
implementation pass.

- **DRIFT-1.1** — verdict: resolved — `modules/app/module.tsx` was not in the original
  Files-to-touch. The Phase-5 infra-integration walk found the `/setup` route applies
  `layout: BlankLayout` at the router level, which would double the `main` landmark and race
  two `useMetaThemeColor` hooks once SetupPage also renders `AuthScreenLayout`. Removed
  `layout: BlankLayout` (and the now-unused import) from the `/setup` route; PLAN.md
  Files-to-touch amended to include `module.tsx`. Verified by TEST-4/TEST-2 asserting exactly
  ONE `main` landmark.
- **DRIFT-1.2** — verdict: resolved — the `auth-page-layout` testid (formerly on the antd
  `Layout` in AuthPage) was dropped when the flat `Layout/Content` scaffolding was replaced by
  `AuthScreenLayout`. No test or code references it (grep-confirmed — only the generated
  registry did); `auth-page-content` is preserved on the inner content div. testid-registry
  regenerated. No behavioral impact.
- **DRIFT-1.3** — verdict: none — the backdrop's raw hex (`#02365b` / `#020a12`) was replaced by
  `var(--auth-backdrop)` exactly per DEC-4; the `data-allow-custom-color` opt-out is retained on
  the (image-bearing, inline-style) decorative divs, which the color-lint requires regardless of
  whether the value is a token — matches the pre-existing `SetupBackdrop` precedent.
- **DRIFT-1.4** — verdict: none — `--auth-backdrop` was added to `:root` + `.dark` only (not
  `@theme inline`), so `gen:design-spec` produced no token-table change; `check:design-spec`
  passes. Confirmed the whole `npm run check` chain is green (tsc + biome + lint:colors +
  check:design-spec + check:gallery-coverage + check:state-matrix + check:testid-registry + …).

**Unresolved drifts:** 0
