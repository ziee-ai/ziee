# Chunk sdk-fe-batteries — TESTS (added; nothing moved)

Additive chunk → no test was relocated. New smokes cover the new surfaces; they
run standalone via the `scripts/ts-resolve.mjs` resolver under
`node --test --experimental-strip-types`.

- **T-FE2-1** [added→sdk] file: `sdk/packages/framework/src/api-client/auth-token.test.ts` covers: default `getAuthToken` reads `localStorage['auth-storage']` `{state:{token}}` (ziee backward-compat).
- **T-FE2-2** [added→sdk] same file covers: default returns null on absent / corrupt `auth-storage` (no crash).
- **T-FE2-3** [added→sdk] same file covers: `setAuthTokenProvider` overrides the default; `null` restores it.
- **T-FE2-4** [added→sdk] same file covers: `setAuthToken(t)` sets a static token; `setAuthToken(null)` clears it. → 4/4 PASS.
- **T-FE1-1** [added→sdk] file: `sdk/packages/framework/src/router/config.test.ts` covers: router config sensible defaults (loginPath `/auth`, homePath `/`, fallback null, gate null).
- **T-FE1-2** [added→sdk] same file covers: `setRouterConfig` merges partial overrides, untouched keys preserved.
- **T-FE1-3** [added→sdk] same file covers: an injected `RoutePermissionGate` is invoked (allow → children, deny → replacement). → 3/3 PASS.
- **T-FE1-type** [added→sdk] the full router (`RouterComponent.tsx` JSX + react-router-dom + slot/store wiring) is type-proven by `tsc --noEmit` on the framework package (exit 0).
- **T-FE3-1** [added→sdk] file: `sdk/packages/kit/scripts/fe3-tailwind-proof.mjs` covers: the `kit.css` `@source` glob resolves to the kit `src/**`, a real `@tailwindcss/node` compile + oxide `Scanner` scan yields `bg-primary`/`text-muted-foreground`/`rounded-md`, and the built CSS emits `.bg-primary` + the `--primary` token. → PASS.

## Backward-compat regression evidence (the additive-chunk anchor)
- ziee `ui/` `tsc --noEmit` — exit 0 (== pre-change baseline).
- ziee `desktop/ui/` `tsc --noEmit` — exit 0 (== pre-change baseline).
- ziee `ui/` `vite build` (build:nocheck) — exit 0 ("✓ built in 2.89s").
- `@ziee/framework` + `@ziee/kit` standalone `tsc --noEmit` — exit 0 each.
