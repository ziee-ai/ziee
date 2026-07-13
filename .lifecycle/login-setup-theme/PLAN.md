# PLAN — login + setup page theming & background (login-setup-theme)

## Context / problem

There are exactly TWO unauthenticated, full-screen, centered-card pages:

- **Login/Register** — `modules/auth/AuthPage.tsx` (renders `LoginForm` / `RegisterForm`).
  It wraps content in `BlankLayoutComponent`, which paints a **flat** `var(--background)`
  to the edges. It has **NO theme toggle** (a signed-out user cannot switch light/dark)
  and **NO decorative themed backdrop**.
- **First-run setup** — `modules/app/SetupPage.tsx`. It ALREADY has the polished
  treatment: a `SetupBackdrop` (cloud raster `setup-clouds.webp` + navy fallback +
  a dark-mode darkening mask) and a `SetupThemeSwitcher` (ghost Sun/Moon button →
  `useTheme().setTheme`). BUT that chrome is **page-local** (not reused), and SetupPage
  does NOT use `BlankLayout`, so it lacks the `main` landmark + `meta[theme-color]`
  management that BlankLayout gives the auth page.

Net: the two sibling pages are inconsistent, the login page is missing the toggle +
backdrop entirely, and the nice chrome is duplicated/non-reusable.

**Product direction (human-chosen via picker, 2026-07-13): "Twin of setup (shared
backdrop)"** — extract ONE shared "auth-screen" chrome and use it on BOTH pages so
login becomes a visual twin of setup; both get the toggle; both adapt light/dark.

The theme mechanism is REUSED, never re-invented: `ConfigClient` store (persisted
preference) → `ThemeProvider` (toggles `dark`/`light` on `<html>`) → `useTheme()`.
Same path `ThemeSettings.tsx` uses. (Human ask #1 explicitly: do NOT hand-roll.)

## Items

- **ITEM-1**: Shared `AuthThemeToggle` component (`modules/auth/AuthThemeToggle.tsx`) — a
  before-sign-in light/dark toggle that mirrors the existing `SetupThemeSwitcher`
  VERBATIM in mechanism (`useTheme().setTheme(isDarkMode ? 'light' : 'dark')`, ghost
  `Button size="icon"`, `Sun`/`Moon` from lucide, `aria-label`). Takes a `data-testid`
  prop so each host keeps a stable id (login → `auth-theme-toggle`; setup keeps its
  existing `app-setup-theme-toggle`). Single source of truth for the toggle. (Human ask #1)
- **ITEM-2**: Shared `AuthScreenLayout` component (`modules/auth/AuthScreenLayout.tsx`) — the
  ONE unauthenticated-screen chrome: renders the themed cloud backdrop (image + dark
  overlay), hosts the `AuthThemeToggle` (top-right), provides the `main` landmark and
  `useMetaThemeColor('--auth-backdrop')` (folding in BlankLayout's semantics + root-bg
  flash guard), and the centered flex container (`relative min-h-dvh flex items-center
  justify-center overflow-hidden p-4`). `children` = the page's card/content. Backdrop
  element carries `data-testid="auth-screen-backdrop"`. (Human ask #2)
- **ITEM-3**: `AuthPage.tsx` adopts `AuthScreenLayout` → the login/register page gains the
  toggle + themed cloud backdrop and becomes a visual twin of setup. Removes the old
  `BlankLayoutComponent` + flat `Layout/Content` scaffolding (its landmark + meta-color
  role is now provided by `AuthScreenLayout`). Title + `LoginForm`/`RegisterForm` and all
  existing `auth-*` testids preserved. (Human asks #1 + #2, on login)
- **ITEM-4**: `SetupPage.tsx` refactored to use the SAME `AuthScreenLayout` — deletes the
  page-local `SetupBackdrop` + `SetupThemeSwitcher`, keeps the setup `Card`/form. Setup
  thereby GAINS the `main` landmark + `meta[theme-color]` it currently lacks, drops the
  duplicated chrome, and stays a pixel-twin of login. `app-setup-theme-toggle` testid
  preserved via the toggle's prop. (Human ask #3: same-issue audit + consistency)
- **ITEM-5**: Backdrop light/dark correctness — add an `--auth-backdrop` semantic token to
  `index.css` (light = the navy sky edge color; dark = the effective darkened edge color),
  drive the backdrop bg-fallback AND `meta[theme-color]` from it (replacing the raw-hex
  `data-allow-custom-color` escape hatch), regenerate `DESIGN_SYSTEM.md` via
  `gen:design-spec`. Result: the screen-edge color follows the theme, and card/text
  contrast over the backdrop passes AA in both themes. (Human ask #2: "render correctly
  and follow the theme")
- **ITEM-7**: (iteration round 1, FB-5) Unify header placement — the login/register heading is
  rendered INSIDE its card (mirroring SetupPage's in-card header), NOT as a sibling above the
  card. Removes `AuthPage`'s external title block; adds an in-card `Title` to `LoginForm`; aligns
  `RegisterForm`'s existing in-card title level. Also eliminates the pre-existing register
  double-header.
- **ITEM-6**: Responsive fidelity — both pages verified at ~390px / tablet / desktop: no
  horizontal page scroll, card `w-full max-w-md`, backdrop full-bleed, toggle tap target
  ≥ 40px, toggle never overlaps the card. Add a narrow-viewport (390px) gallery state so
  `gate:ui` enforces it. (UI-surface checklist: responsive)

## Files to touch

- `src-app/ui/src/modules/auth/AuthThemeToggle.tsx` — NEW (ITEM-1)
- `src-app/ui/src/modules/auth/AuthScreenLayout.tsx` — NEW (ITEM-2)
- `src-app/ui/src/modules/auth/auth-clouds.webp` — NEW (git-mv of `modules/app/setup-clouds.webp`; ITEM-2/5)
- `src-app/ui/src/modules/auth/AuthPage.tsx` — EDIT (ITEM-3)
- `src-app/ui/src/modules/app/SetupPage.tsx` — EDIT (ITEM-4)
- `src-app/ui/src/modules/app/module.tsx` — EDIT: remove `layout: BlankLayout` from the `/setup`
  route so `AuthScreenLayout` is the sole chrome (else two `main` landmarks + two meta-color
  hooks). Discovered by the Phase-5 infra-integration walk. (ITEM-4)
- `src-app/ui/src/index.css` — EDIT: add `--auth-backdrop` token, light + dark (ITEM-5)
- `DESIGN_SYSTEM.md` — REGEN via `npm run gen:design-spec` (ITEM-5)
- `src-app/ui/src/dev/gallery/coverage.ts` — EDIT: coverage entries for the 2 new components + narrow-viewport state note (ITEM-2/6); plus any `*.generated.ts` regen from `gen:gallery-coverage`/`gen:state-matrix`
- `src-app/ui/tests/e2e/auth/login-theme-toggle.spec.ts` — NEW (TEST-1)
- `src-app/ui/tests/e2e/auth/login-backdrop.spec.ts` — NEW (TEST-2)
- `src-app/ui/tests/e2e/auth/auth-backdrop-theme.spec.ts` — NEW (TEST-5)
- `src-app/ui/tests/e2e/auth/auth-responsive.spec.ts` — NEW (TEST-6)
- `src-app/ui/tests/e2e/setup/setup-theme-toggle.spec.ts` — NEW (TEST-3)
- `src-app/ui/tests/e2e/setup/setup.spec.ts` — EDIT: add backdrop + main-landmark assertions (TEST-4)
- `src-app/ui/src/index.css` token unit test — NEW `src-app/ui/src/...` (TEST-7)

NOTE: **desktop workspace is NOT touched** — `src-app/desktop/ui/src/modules/auth` and
`.../app` contain no `AuthPage`/`SetupPage` (desktop uses auto-login and never shows these
pages). Single-workspace (`src-app/ui`) frontend diff. No backend, no migration, no OpenAPI.

## Patterns to follow

- **Theme toggle mechanism** — mirror `modules/app/SetupPage.tsx::SetupThemeSwitcher` and
  `modules/settings-general/components/ThemeSettings.tsx`: both go through
  `ConfigClient`/`ThemeProvider`/`useTheme`. This is the existing, only theme mechanism —
  reuse it, do not add a parallel one.
- **Backdrop** — mirror `modules/app/SetupPage.tsx::SetupBackdrop` (image + dark overlay
  structure), promoted into `AuthScreenLayout`.
- **Unauthenticated screen chrome** — mirror `modules/layouts/blank/BlankLayout.tsx` for the
  `main` landmark (`display: contents`), the `useMetaThemeColor(...)` call, and the
  `useLayoutEffect` root-bg flash guard; fold these into `AuthScreenLayout`.
- **Design token** — mirror how `src/index.css` declares semantic tokens under `:root`/`.dark`
  and how `gen:design-spec` regenerates `DESIGN_SYSTEM.md` (the `check:design-spec` gate).
- **Gallery coverage** — mirror the existing `'via'` / `'flow'` / `'nonvisual'` entries in
  `src/dev/gallery/coverage.ts` (e.g. `modules/auth/LoginForm` = `'via'`).
- **E2E** — mirror `tests/e2e/auth/auth.spec.ts` (`setupAdminThenAuthPage` helper, `byTestId`,
  `tests/utils/theme.ts` `setTheme`/`isDarkMode`, `assertNoAccessibilityViolations`) and
  `tests/e2e/setup/setup.spec.ts`.

## UI-surface checklist (both pages)

- **Precedent** — AuthPage is the twin of SetupPage (both unauthenticated full-screen
  centered-card). After this work they render through the identical `AuthScreenLayout`;
  divergence between them is a bug.
- **Scale / cardinality** — N/A: neither page renders a list/collection (fixed form fields +
  a bounded provider-button list already handled by `ProviderButtons`). No paging needed.
- **Device size / responsive** — card `w-full max-w-md` + `p-4`; backdrop full-bleed;
  toggle pinned `absolute right-4 top-4`. Behavior identical at 390px/tablet/desktop
  (single-column card, no reflow). Covered by ITEM-6 + a 390px gallery state.
- **User-visible progress** — the toggle is instant-apply (no spinner needed); form submit
  already shows `loading` on the submit button (unchanged).
- **Input economy** — N/A (no new inputs; the toggle is a two-state control, not a text field).
- **JTBD** — see below.
- **Multi-instance** — N/A (single top-level route each; not a split/pane surface).
- **Platform-provided affordances** — the toggle is app chrome (no platform equivalent for a
  signed-out theme switch); rendered identically on web + desktop-web. No platform gating needed.

## JTBD (jobs-to-be-done) — what a real human wants

- **Signed-out user on login**: "I prefer dark mode; let me flip it BEFORE I sign in so the
  login screen (and the app I'm about to enter) isn't blinding." → visible top-right toggle,
  one click, whole screen + backdrop + card flip instantly and the choice persists into the app.
- **Signed-out user, aesthetics**: "The sign-in screen should look intentional, not a bare
  form on a flat slab." → themed cloud backdrop, same as the setup screen, that reads well in
  both light and dark (no muddy/low-contrast dark state).
- **First-run admin on setup**: same two jobs, plus "the two screens I see first (setup then
  login) should feel like one product." → identical chrome via `AuthScreenLayout`.
- **Empty/error/loading**: auth error alert + provider-load spinner/error already handled by
  `LoginForm`/`ProviderButtons`; setup error alert already handled — all render over the same
  backdrop, unchanged. Mobile (390px) covered above.
