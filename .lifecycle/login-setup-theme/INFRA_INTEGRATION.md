# INFRA_INTEGRATION.md — the two mandatory Phase-5 walks

## User-experience walk (per item)

- **ITEM-1/3 (login toggle):** signed-out user lands on `/auth`, sees a Sun/Moon button
  top-right, clicks it → the whole screen (backdrop + card) flips instantly; the choice
  persists (localStorage `config-client-storage`) and carries into the app after sign-in.
  Register mode shares the same chrome. No layout shift (toggle is absolutely positioned).
- **ITEM-4 (setup toggle):** first-run admin on `/setup` gets the identical control/behavior.
- **ITEM-2/5 (backdrop):** both screens show the themed cloud backdrop; in dark mode a
  darkening layer keeps the bright cloud from glaring; iOS status/nav bar tint matches the
  backdrop edge (not a mismatched white/near-black strip).
- **ITEM-6 (responsive):** at 390px the card stays `max-w-md` centered with `p-4` gutters,
  backdrop full-bleed, toggle in the corner not overlapping the card.

## Infrastructure-integration walk (every subsystem the change touches)

- **Router layout system** — CRITICAL FINDING: `/setup` route registers `layout: BlankLayout`
  (`modules/app/module.tsx:41`); `/auth` does NOT (AuthPage self-wraps `BlankLayoutComponent`).
  `AuthScreenLayout` provides its OWN `<main>` + `useMetaThemeColor` + root-bg guard. If setup
  keeps the router `layout: BlankLayout` AND renders `AuthScreenLayout`, we get (a) two `main`
  landmarks (axe a11y violation) and (b) two `useMetaThemeColor` hooks fighting (--background vs
  --auth-backdrop). RESOLUTION: remove `layout: BlankLayout` from the `/setup` route so
  `AuthScreenLayout` is the sole chrome — mirroring how `/auth` works. Adds
  `modules/app/module.tsx` to Files-to-touch (plan amended; recorded as DRIFT-1 impl-wins-ish,
  discovered pre-implementation by this walk).
- **ThemeProvider / ConfigClient** — the toggle calls `useTheme().setTheme`, which is
  `ConfigClient.setThemePreference`; ThemeProvider's effect toggles `.dark`/`.light` on `<html>`
  and re-applies the accent. No change to that pipeline (reuse only). The toggle button must be
  rendered UNDER `ThemeProvider` — both `/auth` and `/setup` render inside the app tree where
  ThemeProvider is mounted (App root), so `useTheme()` resolves. Verified: SetupThemeSwitcher
  already calls `useTheme()` on `/setup` today, proving the provider is in scope there.
- **meta[theme-color] / iOS bars** — `useMetaThemeColor('--auth-backdrop')` reads the token via
  `getComputedStyle`; rAF-defers the read so it lands after ThemeProvider toggles the class
  (documented one-step-behind hazard). Single owner now (BlankLayout no longer runs for these
  routes).
- **Accessibility (axe / gallery runtime-health)** — exactly one `main` landmark per page after
  the router-layout fix. Toggle carries an `aria-label` (Switch to light/dark). Card text sits on
  opaque `--card`, so backdrop contrast doesn't affect text AA; axe verifies (TEST-5).
- **Gallery / state-matrix** — two new components (`AuthScreenLayout`, `AuthThemeToggle`) need
  `coverage.ts` entries; a 390px narrow state for the flow pages is added so `gate:ui` enforces
  responsive. Regen via `gen:gallery-coverage` / `gen:state-matrix`.
- **testid-registry** — new ids `auth-theme-toggle`, `auth-screen-backdrop`; existing
  `app-setup-theme-toggle` preserved. Regen via `gen:testid-registry`.
- **lint:colors** — the backdrop uses inline `style` color keys → keeps `data-allow-custom-color`
  (legitimate decorative surface), but the raw `#02365b`/`#020a12` hex are replaced by
  `var(--auth-backdrop)` (DEC-4).
- **Design-spec (`check:design-spec`)** — `--auth-backdrop` lives in `:root`/`.dark` only (NOT in
  `@theme inline`), so the generated token TABLE is unchanged; still run `gen:design-spec` to be
  safe. No `bg-auth-backdrop` utility is needed (used only via `var()`).
- **Desktop UI** — no `AuthPage`/`SetupPage` there (auto-login); nothing to mirror. R2-3 N/A.
- **E2E suites** — the existing `tests/e2e/auth/*` (many) + `tests/e2e/setup/*` (16) are the
  regression backstop; all `auth-*` / `app-setup-*` testids preserved so they keep passing.
