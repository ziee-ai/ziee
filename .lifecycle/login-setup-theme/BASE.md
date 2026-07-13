# BASE.md — conflict surface vs current main (login-setup-theme)

Branch cut from `origin/main` @ 482a9cd05.

## Migrations
- Highest existing migration: `00000000000157_remove_unused_builtin_mcp_servers.sql`.
- **This branch adds NO migration** (pure frontend). No migration-number collision possible.

## OpenAPI / types regen
- **None.** No backend type change; no `openapi.json` / `api-client/types.ts` regen implied.

## Files this branch touches that main may also be changing
- `src-app/ui/src/modules/auth/AuthPage.tsx` — stable; not a hot file on main.
- `src-app/ui/src/modules/app/SetupPage.tsx` — recently touched (email-regex fix), low churn.
- `src-app/ui/src/index.css` — shared design-token file; **append-only** change (add one token
  block), low collision risk; regenerates `DESIGN_SYSTEM.md`.
- `src-app/ui/src/dev/gallery/coverage.ts` + `*.generated.ts` — additive coverage entries.
- New files (`AuthThemeToggle.tsx`, `AuthScreenLayout.tsx`, `auth-clouds.webp`, e2e specs) —
  no collision.
- `git mv` of `modules/app/setup-clouds.webp` → `modules/auth/auth-clouds.webp` — verify no
  other importer of `setup-clouds.webp` remains (only `SetupPage.tsx` imports it today).

## Regen commands implied at merge time
- `npm run gen:design-spec` (DESIGN_SYSTEM.md), `npm run gen:gallery-coverage`,
  `npm run gen:state-matrix` — all inside `src-app/ui`. No `just openapi-regen`.
