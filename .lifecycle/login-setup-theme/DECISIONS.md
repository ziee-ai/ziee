# DECISIONS.md — login-setup-theme

### DEC-1: Visual direction for the login themed background?
**Resolution:** "Twin of setup (shared backdrop)" — extract ONE shared auth-screen chrome
(backdrop + toggle) and use it on BOTH login and setup; login becomes a visual twin of setup.
**Basis:** user — explicit option-picker question answered 2026-07-13 (chosen over
"token gradient on both" and "login stays plain + toggle only").

### DEC-2: Theme mechanism — reuse or new?
**Resolution:** Reuse the existing `ConfigClient` store + `ThemeProvider` + `useTheme()` path
(identical to `ThemeSettings.tsx` and `SetupThemeSwitcher`). No new theme mechanism, no new store.
**Basis:** user + convention — human request #1 states "do NOT hand-roll a new theme mechanism —
mirror how the app already toggles theme"; `ThemeSettings.tsx` is the named precedent.

### DEC-3: Where does the shared chrome live, and how do both pages share it?
**Resolution:** New components in `modules/auth/` (`AuthScreenLayout.tsx`, `AuthThemeToggle.tsx`);
`SetupPage` (in `modules/app/`) imports from `modules/auth/`. The cloud asset is `git mv`-d to
`modules/auth/auth-clouds.webp`. One layout wraps each page's card via `children`.
**Basis:** convention — auth owns the sign-in surface; cross-module component import is already
used across modules. Keeps a single source of truth (affordance-parity/reuse rule).

### DEC-4: Backdrop edge color — fixed constant, or admin-configurable settings row?
**Resolution:** FIXED design token `--auth-backdrop` in `index.css` (light + dark values), NOT an
admin setting. It is a brand/decorative color, not an operational tunable (no resource limit /
retention / quota / toggle semantics). Structured as a semantic CSS token (promotable later),
never inline magic hex — replacing the current `data-allow-custom-color` raw-hex escape hatch.
**Basis:** convention — the app's other decorative surface colors (`--sidebar`, `--card`, …) are
fixed tokens in `index.css`, not settings rows. The Phase-4 configurable-settings rule applies to
OPERATIONAL tunables; a backdrop color is neither.

### DEC-5: `--auth-backdrop` dark value — must AA contrast hold over it?
**Resolution:** Choose the light value = the navy sky edge color (~`#02365b`, matching today's
`SetupBackdrop` fallback) and the dark value = the effective darkened edge color the current dark
mask produces (~very-dark navy). The card is an opaque `--card` surface, so text sits on `--card`
(already AA-verified by the design system), NOT directly on the backdrop; axe (TEST-5) verifies AA
in both themes as the backstop. `meta[theme-color]` = `--auth-backdrop`.
**Basis:** codebase — reuses the existing `SetupBackdrop` colors; card opacity already guarantees
text contrast; axe is the enforcement.

### DEC-6: Toggle testid — one shared id or per-host?
**Resolution:** (amended in Phase-7 after the blind audit — see DRIFT-2) ONE shared literal id
`auth-theme-toggle` rendered directly on the `AuthThemeToggle` element (no `data-testid` prop).
A page only ever shows one toggle, so `byTestId(page,'auth-theme-toggle')` is unambiguous per
page. The earlier per-host dynamic-prop scheme was dropped because a dynamic `data-testid={prop}`
is invisible to the regex-based testid-registry generator, so BOTH toggle ids fell out of the
typed registry (audit finding, two angles). `app-setup-theme-toggle` was a fresh id (added with
the setup backdrop; NO test in origin/main referenced it — verified via `git grep`), so
retiring it is safe.
**Basis:** convention + audit — a static literal keeps the toggle in the typed registry (the
codebase pattern); no existing test depended on the old setup-specific id.

### DEC-7: Does `AuthScreenLayout` keep BlankLayout's `main` landmark + meta-color + flash guard?
**Resolution:** Yes — fold all three (`display:contents` main landmark, `useMetaThemeColor`, the
`useLayoutEffect` root-bg guard) into `AuthScreenLayout`. `BlankLayoutComponent` itself is NOT
removed (other pages still use it). Setup, which previously lacked these, gains them.
**Basis:** codebase — `BlankLayout.tsx` is the precedent; avoids an a11y/flash regression on auth
and an a11y improvement on setup.

### DEC-8: Meta-theme-color CSS var — `--background` or `--auth-backdrop`?
**Resolution:** `--auth-backdrop` (the color actually painted to the screen edges by the full-bleed
backdrop), NOT `--background` (which the backdrop now covers). Fixes the iOS status-bar mismatch.
**Basis:** codebase — `useMetaThemeColor`'s own doc-comment: layouts pass "the CSS var of the
surface at the screen edges"; here that surface is the backdrop.

### DEC-9: Register `AuthScreenLayout` as a route/layout, or a plain component?
**Resolution:** Plain component composed inside the two page components — NOT a router layout entry.
**Basis:** convention — `BlankLayout` is likewise consumed as a component (`BlankLayoutComponent`),
not a nested route element, for these two standalone pages.
