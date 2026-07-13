# HUMAN_FEEDBACK.md — login-setup-theme

Living ledger of human feedback (verbatim), from feature kickoff onward. The three
kickoff asks are recorded verbatim below; they are the spec this feature is built to.
They flip to `resolved` only once the covering test passes (Phase 8), so they remain
`open` while the plan awaits approval / implementation.

- **FB-1** [status: open] — "bring dark mode on and off" — the login page currently has NO way to toggle dark/light before you sign in; add a dark-mode ON/OFF toggle. Reuse the existing theme precedent `src-app/ui/src/modules/settings-general/components/ThemeSettings.tsx` (do NOT hand-roll a new theme mechanism — mirror how the app already toggles theme). → PLAN ITEM-1 (`AuthThemeToggle`, reuses `useTheme`/`ConfigClient`) + ITEM-3 (login adopts it); covered by TEST-1/TEST-3 (real-click toggles `html.dark`). [generalizable: no — feature-specific surface, but the "reuse the existing theme mechanism, never hand-roll" instinct is already an established convention]
- **FB-2** [status: open] — "the background over" — make the login pages themed BACKGROUND render correctly and follow the theme (audit how the background looks in dark vs light and fix it so the proper app/themed background applies to the login page). → PLAN ITEM-2 (shared themed backdrop) + ITEM-5 (`--auth-backdrop` token + meta-color follow the theme; AA in both); covered by TEST-2/TEST-5. [generalizable: no]
- **FB-3** [status: open] — "check the setup page" — the first-run setup page `src-app/ui/src/modules/app/SetupPage.tsx`; audit it for the SAME issues (dark-mode toggle, themed background) AND for precedent/layout/responsive consistency, and fix. → Audit finding: setup ALREADY had a page-local toggle + backdrop but LACKED the `main` landmark + `meta[theme-color]` and DUPLICATED the chrome; PLAN ITEM-4 refactors it onto the shared `AuthScreenLayout` (gains landmark + meta-color, drops duplication, stays a twin of login); ITEM-6 covers responsive. Covered by TEST-3/TEST-4/TEST-6. [generalizable: yes — when asked to "check X for the same issues", audit X against its sibling for MISSING shared infrastructure (landmark, meta-color) too, not only the named symptom]
- **FB-4** [status: resolved] — (plan-time picker) login themed-background visual direction. → Human chose "Twin of setup (shared backdrop)" (2026-07-13); recorded as DEC-1 and drives the whole plan shape. [generalizable: no]

## Notes
- No running feature to review yet — this ledger is seeded from the verbatim kickoff asks
  and will accrue any review feedback once the feature is testable. FB-1..FB-3 stay `open`
  until their covering tests pass at Phase 8.
