# DRIFT-3 — iteration round 1 (FB-5: header inside the card)

- **DRIFT-3.1** — verdict: none — login heading moved into `LoginForm`'s card (`Title level={2}`
  "Welcome back"), `RegisterForm` title aligned to level 2, `AuthPage` external title block +
  now-unused `Title` import removed. Matches DEC-10 and the SetupPage precedent. tsc + full
  `npm run check` green.
- **DRIFT-3.2** — verdict: none — removing the external title also eliminated the pre-existing
  register DOUBLE header (external "Create your account" + in-card "Create Account"), which the
  Phase-6 audit had flagged as pre-existing. One heading per screen now.

**Unresolved drifts:** 0
