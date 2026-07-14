# Chunk sdk-fe-batteries — DRIFT round 1

Reconciliation of the implemented diff against `CUT.md`/`TRANSFORMS.md` and the
backward-compat tripwires (the equivalence anchor for an additive chunk).

- **DRIFT-1.1** — Every `CUT.md` file exists and every declared public symbol is
  exported: `setAuthToken`/`setAuthTokenProvider` (re-exported from
  `api-client/index.ts`), `createRouterModule`/`RouterComponent`/`Routes`/
  `useRoutesStore`/`LazyRouteRenderer`/`setRouterConfig`/`getRouterConfig` +
  types (from `router/index.ts`), `@ziee/kit/styles/kit.css` (in kit
  `package.json` exports). — verdict: none

- **DRIFT-1.2** — Every non-trivial change is declared in `TRANSFORMS.md`
  (T-FE2-1/2, T-FE1-1..5, T-FE3-1/2, T-INFRA-1). No undeclared surface. — verdict: none

- **DRIFT-1.3** — **Backward-compat tripwire GREEN.** Both consuming workspaces
  `tsc --noEmit` clean AFTER the changes (ziee `ui/` exit 0, `desktop/ui/` exit 0
  — matching their pre-change baselines, also exit 0), and ziee `ui/` `vite build`
  exits 0 ("✓ built in 2.89s"). The framework package + kit package each
  `tsc --noEmit` clean standalone. — verdict: none

- **DRIFT-1.4** — **No codegen / `types.ts` impact.** The full diff touches only
  `sdk/packages/{framework,kit}/**` (frontend packages) — no Rust, no
  `server/src/openapi/emit_ts.rs`, no `openapi/openapi.json`, no
  `api-client/types.ts`. `git diff --name-only` contains zero generated-artifact
  paths. The golden `types.ts` is therefore untouched (STOP condition not
  triggered). — verdict: none

- **DRIFT-1.5** — **Tree-shakeability preserved.** `router` is NOT re-exported
  from `framework/src/index.ts` (grepped: the main barrel exports only
  module-system/module/stores/events/overrides — router absent). react-router-dom
  is `optional` in `peerDependenciesMeta`, so an app that never imports
  `@ziee/framework/router` pulls no react-router. — verdict: resolved

- **DRIFT-1.6** — **Default-path equivalence (FE-2).** `defaultAuthToken` is the
  old `getAuthToken` body verbatim (same `auth-storage` key, same `{state:{token}}`
  parse, same corrupt-JSON `console.error` + null return) plus a `typeof
  localStorage` guard that is a no-op in the browser. With no provider registered
  (ziee's state) `getAuthToken()` returns exactly what it did before. Proven by
  TEST-FE2-1/2. — verdict: resolved

- **DRIFT-1.7** — **Router domain-decoupling complete.** `grep` of
  `sdk/packages/framework/src/router/**` shows zero `@/`, zero `@ziee/kit`, zero
  `PermissionExpr`/`usePermission` imports; the only external import is
  `react-router-dom` (the optional peer) + intra-package relative imports. — verdict: resolved

**Unresolved drifts:** 0
