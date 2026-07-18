# Chunk sdk-fe-batteries — TRANSFORMS (every non-trivial change + rationale)

This chunk is ADDITIVE, so "transforms" here = the deliberate design choices for
each new/edited surface. Each of the three fixes carries a Decision.

## FE-2 — injectable auth token

- **T-FE2-1** `getAuthToken` is split into `getAuthToken()` (dispatcher) + `defaultAuthToken()` (the verbatim old localStorage body) + a module-level `authTokenProvider` holder, with public setters `setAuthTokenProvider(fn)` and `setAuthToken(token)`. — **why:** the old `getAuthToken` hard-coded `localStorage['auth-storage']` with the `{state:{token}}` zustand-persist shape and had NO setter, so any app not using that exact store silently sent every request unauthenticated (the HIGH leak). The dispatcher calls the injected provider if set, else the default — so ziee (which uses `auth-storage`) is byte-for-byte unaffected, and a foreign app calls `setAuthToken(...)` once and its requests authenticate.
- **T-FE2-2** `defaultAuthToken` guards `typeof localStorage !== 'undefined'`. — **why:** makes the default SSR/node-safe (returns null instead of a ReferenceError) so the module imports cleanly under `node --test`; in a browser `localStorage` always exists, so ziee's behavior is unchanged.

### Decision (FE-2)
**Question:** setter-only, provider-only, or both? And does the default stay?
**Resolution:** ship BOTH `setAuthToken(token)` (static convenience — the common
"I fetched a token, use it" case) and `setAuthTokenProvider(fn)` (dynamic — read
from any store/keychain on each request), because CytoAnalyst wanted the simple
setter but a live app needs the per-request provider. The DEFAULT localStorage
path is retained verbatim and used whenever no provider is injected, so ziee is a
strict superset — backward-compatible. `null` restores the default (provider) /
clears the token (setter). Zero TBD.

## FE-1 — optional router subpath

- **T-FE1-1** the router is a domain-agnostic port of ziee's `ui/src/modules/router`; every ziee coupling is severed: `usePermission`/`PermissionExpr` → an injected `RoutePermissionGate`; `@ziee/kit` `Button`/`Result` 403 panel → the app's gate owns its denied UI; `@/core/components/{LazyComponentRenderer,Loading}` → a self-contained `LazyRouteRenderer` + an injectable `fallback`; `@/utils/lazyWithPreload` → the router registers `<RouterComponent/>` as an eager element (no lazy-util dependency); the hardcoded `/auth` + `/` redirects → configurable `loginPath`/`homePath`. — **why:** the SDK must not name any app domain type; ziee's router imports 5 app-internal modules, none of which can ship in a reusable package.
- **T-FE1-2** `RouteConfig.permission` is typed `unknown` (was ziee's `PermissionExpr`). — **why:** the SDK owns no permission model; the app narrows it inside its injected gate. When no gate is registered, `permission` is ignored with a one-time `console.warn` (a generic router cannot fail-closed on a model it doesn't understand).
- **T-FE1-3** delivered as `createRouterModule(options)` FACTORY (ziee uses a bare `createModule({...})` default export). — **why:** app-specific knobs (redirect paths / fallback / permission gate) must reach the lazy `RouterComponent`, which reads them from a module-level config holder written once by the factory (same DI shape as the api-client's `setBaseUrlResolver`). A factory is the clean opt-in seam; ziee need not adopt it.
- **T-FE1-4** `react-router-dom` added as an **optional** `peerDependency` (`peerDependenciesMeta.optional`), and the `./router` export added to `package.json` `exports` — NOT re-exported from the main `index.ts` barrel. — **why:** keeps react-router out of the dependency graph of any app (and ziee) that doesn't route; the subpath is the tree-shakeable opt-in boundary.
- **T-FE1-5** the `routeGuards` fail-closed seal is preserved verbatim from ziee (empty slot + protected routes ⇒ `<Navigate to={loginPath}/>`), and `routerEffects` mount inside `<BrowserRouter>`. — **why:** the guard is a security control; a generic router must keep the same fail-closed posture ziee has (protected content never renders ungated).

### Decision (FE-1)
**Question:** can ziee's router be cleanly extracted, or is it too coupled?
**Resolution:** cleanly extractable — the coupling is all at the LEAVES (permission
gate, kit 403 UI, loading spinner, lazy-preload util, redirect literals), each
replaced by an injection point with a sensible default. The CORE mechanism
(module `routes` declaration-merge → `Routes` store collection → layout grouping →
guard composition → `routerEffects`) is domain-agnostic and ported verbatim. It is
shipped as an OPTIONAL subpath; ziee is NOT migrated onto it in this chunk (its
router keeps its richer app-coupled permission gate). Zero TBD.

## FE-3 — Tailwind v4 kit wiring

- **T-FE3-1** ship a CSS entry `@ziee/kit/styles/kit.css` = `@import "./tokens.css"; @source "../**/*.{ts,tsx}";`, not a JS preset. — **why:** `@ziee/kit`/ziee are Tailwind **v4** (CSS-first; `@tailwindcss/vite`, no `tailwind.config.js`), where a JS preset is the v3 model. The idiomatic v4 fix is a CSS entry: Tailwind resolves `@source` relative to the file that contains it, so the kit-shipped glob always points at the kit's own `src/**` no matter where the consuming app lives — one documented `@import "@ziee/kit/styles/kit.css";` line gives an app both the tokens and the source-scan.
- **T-FE3-2** README documents the one line + a tokens-only advanced path; `./styles/kit.css` added to `package.json` `exports`. — **why:** the gap was "undocumented"; the exact wiring line is now in the package README.

### Decision (FE-3)
**Question:** JS preset or CSS entry?
**Resolution:** CSS entry — v4 is CSS-first, and a preset would be a v3 anachronism
that doesn't solve the actual problem (Tailwind not scanning `node_modules/@ziee/kit`).
The `@source`-relative-to-CSS-file resolution is the mechanism that makes it
one-line and location-independent. Proven functionally (T below). Zero TBD.

## Cross-cutting

- **T-INFRA-1** a minimal `node --test` resolver (`scripts/ts-resolve*.mjs`) maps extensionless relative specifiers to `.ts`/`.tsx`/index so the new smokes run standalone. — **why:** the framework's TS sources use extensionless imports (`allowImportingTsExtensions`); Node ESM needs the extension. Mirrors ziee ui's existing `scripts/node-test-hooks.mjs` pattern. Test-only; not shipped in the package `exports`.

## Functional proofs run
- FE-2: `node --test src/api-client/auth-token.test.ts` → 4/4 pass (default path, corrupt/absent, provider override, static setter+clear).
- FE-1: `node --test src/router/config.test.ts` → 3/3 pass (defaults, partial merge, gate invoke); full router type-proven by `tsc --noEmit` on the package.
- FE-3: `node scripts/fe3-tailwind-proof.mjs` → PASS: kit.css `@source` scanned 6021 candidates from the kit src, emitted 196 KB CSS incl. `.bg-primary` + the `--primary` token (real `@tailwindcss/node` compile + oxide `Scanner`).

Zero unresolved markers; every change carries a rationale + a Decision.
