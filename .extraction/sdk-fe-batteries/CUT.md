# Chunk sdk-fe-batteries — frontend "batteries-included" gaps (CUT manifest)

**This chunk is ADDITIVE, not an extraction/MOVE.** It closes three
batteries-included gaps in the already-extracted `@ziee/framework` + `@ziee/kit`
UI packages, found by a real second consumer (CytoAnalyst) — FE-2, FE-1, FE-3
from `cytoanalyst/SDK_GAPS.md`. Nothing is moved out of ziee; every change is a
new export / new optional subpath / new CSS entry, all backward-compatible so
ziee's own `ui/` + `desktop/ui/` keep working with **zero** code change.

Because nothing moves, the classic extraction checks (`E6 source-absent`,
byte-equivalence tripwires) do not apply — the equivalence anchor here is
**backward-compat**: both consuming UI workspaces still `tsc --noEmit` clean and
ziee's `ui/` still `vite build`s green against the updated packages.

## Files (all NEW or additively edited — no deletions, no moves)

FE-2 — injectable auth token (`@ziee/framework/api-client`):
- edit: `sdk/packages/framework/src/api-client/core.ts` — add `setAuthToken` / `setAuthTokenProvider` + an injectable `getAuthToken`; default localStorage path preserved verbatim.
- edit: `sdk/packages/framework/src/api-client/index.ts` — re-export the two setters.
- add: `sdk/packages/framework/src/api-client/auth-token.test.ts` — FE-2 smoke (4 tests).

FE-1 — optional router (`@ziee/framework/router`, opt-in subpath):
- add: `sdk/packages/framework/src/router/types.ts` — `RouteConfig`/`LayoutDefinition` + the `CreateModuleOptions.routes` + `Slots` (routerEffects/routeGuards) declaration-merges.
- add: `sdk/packages/framework/src/router/config.ts` — injectable router config DI (loginPath/homePath/fallback/permissionGate).
- add: `sdk/packages/framework/src/router/routes-store.ts` — the `Routes` store + `RegisteredStores` merge.
- add: `sdk/packages/framework/src/router/LazyRouteRenderer.tsx` — dependency-free lazy element materializer.
- add: `sdk/packages/framework/src/router/RouterComponent.tsx` — the `BrowserRouter` + layout/guard grouping renderer.
- add: `sdk/packages/framework/src/router/module.tsx` — `createRouterModule(options)` factory.
- add: `sdk/packages/framework/src/router/index.ts` — the subpath barrel.
- add: `sdk/packages/framework/src/router/config.test.ts` — FE-1 config-DI smoke (3 tests).
- edit: `sdk/packages/framework/package.json` — add the `./router` export + `react-router-dom` as an OPTIONAL peerDependency.

FE-3 — Tailwind v4 wiring for the kit (`@ziee/kit`):
- add: `sdk/packages/kit/src/styles/kit.css` — the one-line wiring entry (`@import tokens.css` + the `@source` glob into the kit's own `src/**`).
- add: `sdk/packages/kit/README.md` — documents the exact one-line wiring.
- edit: `sdk/packages/kit/package.json` — add the `./styles/kit.css` export.
- add: `sdk/packages/kit/scripts/fe3-tailwind-proof.mjs` — FE-3 functional proof (real Tailwind compile + oxide Scanner).

Test infra (framework):
- add: `sdk/packages/framework/scripts/ts-resolve{,.-hooks}.mjs` — a minimal `node --test` extensionless-import resolver (mirrors ziee ui's `node-test-hooks.mjs`), so the new `.test.ts` smokes run standalone under `--experimental-strip-types`.

## Public API added (the whole surface of this chunk)

- `@ziee/framework/api-client`: `setAuthToken(token: string | null)`, `setAuthTokenProvider(fn: (() => string | null) | null)`.
- `@ziee/framework/router` (NEW subpath): `createRouterModule(options?)`, `RouterComponent`, `Routes`, `useRoutesStore`, `LazyRouteRenderer`, `setRouterConfig`, `getRouterConfig`, and the types `RouteConfig`, `LayoutDefinition`, `RouterConfig`, `RoutePermissionGate`, `CreateRouterModuleOptions`. NOT re-exported from the main barrel (tree-shakeable opt-in).
- `@ziee/kit/styles/kit.css` (NEW export): the documented one-line Tailwind v4 wiring.

## Design-gate

**Backward-compat + domain-agnostic.** (1) `getAuthToken`'s default is byte-preserved
so ziee is unaffected; the provider is opt-in DI (same pattern as `setBaseUrlResolver`).
(2) The router is a domain-agnostic port of ziee's own `ui/src/modules/router` with
ALL ziee coupling severed (no `@/core/permissions`, no `@ziee/kit`, no `@/core/components`,
no `@/utils/lazyWithPreload`) — permission gating + loading fallback + redirect paths are
injected. It lives on a SEPARATE subpath so react-router never enters an app (or ziee) that
doesn't opt in. (3) The kit CSS entry is purely additive; ziee keeps importing its own
`index.css` and never touches `kit.css`.

**No OpenAPI / `types.ts` impact:** this chunk touches only frontend packages — no Rust,
no `emit_ts` generator, no codegen. The generated `types.ts` is untouched (verified: no
files under `api-client/types.ts` or `openapi/` are in the diff).
