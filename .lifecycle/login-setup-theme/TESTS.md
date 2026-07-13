# TESTS.md — login-setup-theme

Every ITEM is covered by ≥1 TEST; the UI diff carries `tier: e2e` specs. All
render/behavior claims are proven by e2e that RUNS the surface (B7 — unit here is
pure `node --test`, no RTL, so behavior CANNOT be a unit test). No new permission is
introduced → no `[negative-perm]` spec required (A10 N/A).

- **TEST-1** (tier: e2e) [covers: ITEM-1, ITEM-3] file: `src-app/ui/tests/e2e/auth/login-theme-toggle.spec.ts` — asserts: on the login page `auth-theme-toggle` is visible; CLICKING it flips `document.documentElement.classList` from `light`→`dark` (and back), the persisted `config-client-storage.themePreference` updates, and a re-read after reload keeps the chosen theme. (The real human ask #1, exercised by a real click — not the `setTheme` localStorage helper.)
- **TEST-2** (tier: e2e) [covers: ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/auth/login-backdrop.spec.ts` — asserts: the login page renders the shared `auth-screen-backdrop` element (full-bleed) AND the login card over it; the backdrop is present in both light and dark; the `main` landmark exists (role=main).
- **TEST-3** (tier: e2e) [covers: ITEM-1, ITEM-4] file: `src-app/ui/tests/e2e/setup/setup-theme-toggle.spec.ts` — asserts: on the setup page the shared `auth-theme-toggle` is visible and CLICKING it flips `html` light↔dark (proves the SAME shared toggle works in the setup context via a real click), and the setup form/card remain visible after the flip.
- **TEST-4** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/setup/setup.spec.ts` — asserts: the setup page now renders `auth-screen-backdrop` and exposes a `main` landmark (role=main), and the existing light + dark `assertNoAccessibilityViolations` checks still pass — regression backstop for the shared-layout refactor, added to the existing setup suite.
- **TEST-5** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/auth/auth-backdrop-theme.spec.ts` — asserts: `meta[name="theme-color"]` `content` DIFFERS between light and dark on the login page (edge color follows the theme via `--auth-backdrop`), the computed `--auth-backdrop` value is non-empty and changes light↔dark, and `assertNoAccessibilityViolations` (axe AA contrast) passes over the backdrop in BOTH themes on login AND setup.
- **TEST-6** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/auth/auth-responsive.spec.ts` — asserts: at 390px viewport on login AND setup, `document.documentElement.scrollWidth <= clientWidth` (no horizontal scroll), the card is visible, and the theme-toggle bounding box is ≥ 40×40px and does not intersect the card.
- **TEST-7** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/index.css.auth-backdrop.test.ts` — asserts: reading `src/index.css`, the `--auth-backdrop` custom property is declared in BOTH the light (`:root`) and the `.dark` scope (pure-node string assertion; the deterministic complement to TEST-5's runtime check, and to `check:design-spec`).
- **TEST-8** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/auth/auth-header-in-card.spec.ts` — asserts: on the login page the "Welcome back" heading is a DESCENDANT of `auth-login-card` (inside the card, not a sibling above it); after switching to register, the register heading is a descendant of `auth-register-card`; and there is exactly ONE heading per screen (no external+in-card double header).

## Coverage map (every ITEM → ≥1 TEST)
- ITEM-1 → TEST-1, TEST-3
- ITEM-2 → TEST-2
- ITEM-3 → TEST-1, TEST-2
- ITEM-4 → TEST-3, TEST-4
- ITEM-5 → TEST-5, TEST-7
- ITEM-6 → TEST-6

## Gate lines to record in Phase 8 (TEST_RESULTS.md)
- `npm run check (ui): PASS` (tsc + biome + lint:colors + check:design-spec +
  check:gallery-coverage + check:state-matrix + …).
- `gate:ui (ui): PASS` (A7 boot/runtime canary — runtime-health + Layer A/axe over the
  touched gallery surfaces, incl. the new 390px state).
