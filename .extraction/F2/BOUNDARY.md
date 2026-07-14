# Chunk F2 — BOUNDARY (green evidence)

## What moved

ziee's domain-agnostic frontend RUNTIME — the meta-framework core
(`ui/src/core/*`: module system, `createModule`, EventBus, Stores proxy +
store-kit, the desktop UI-override seam, the realtime sync CLIENT) + the ApiClient
transport (`api-client/core.ts` + `sse-types.ts` + the `createApiClient` factory) —
into `sdk/packages/framework/src/` as the `@ziee/framework` npm workspace package,
equivalence-preserving. 28 files moved (bodies byte-preserved, only import
specifiers rewritten); 3 new infra files (barrel, api-client factory, env shim) +
package.json/tsconfig.

## Gate — tsc-clean (wire-irrelevant chunk)

F2 touches **no** route/type/schema (generated `api-client/types.ts` byte-identical
to baseline), so it is wire-irrelevant and the E8 openapi/types golden does not
apply. The gate is **tsc-clean**, verified:

- `sdk/packages/framework` `npx tsc --noEmit` → **exit 0**
- `src-app/ui` `npx tsc --noEmit` → **exit 0**
- `src-app/desktop/ui` `npx tsc --noEmit` → **exit 0**
- `src-app/ui` `npm run lint:guardrails` (biome noRestrictedImports, 951 files) → **exit 0**

`npm install` at the worktree root wires `node_modules/@ziee/framework` →
`sdk/packages/framework` as a symlink.

## The two design gates the chunk asked to resolve

1. **Generated-types interface** (T-2): the ApiClient runtime is generic
   (`callAsync<TResponse>`, `createApiClient<TClient>(endpoints)`) + the sync
   runtime uses a framework-local generic `SyncEvent`. The per-app generated
   `types.ts` stays per-app OUTPUT and is bound ONLY in ziee's thin
   `api-client/index.ts` and its app-side `sync/types.ts` — the framework names no
   generated type, and ziee re-derives its exact per-endpoint + per-entity typing.
2. **Declaration-merge boundary** (T-3, the audit-flagged fiddly part): CROSSED
   cleanly. The five augmentable interfaces are exposed at stable subpath
   specifiers (`@ziee/framework/{stores,events,module,module-system/types,
   overrides}`); every ziee `declare module` (both `@/core/*` and relative
   `../../core/stores`) rewritten to them; and the framework compiles STANDALONE
   with all five empty via the `SlotKey` conditional + `as never` emit.

## Reported (per the gate's "if it can't be cleanly crossed, report why")

`core/permissions/*`, `core/components/*`, and `core/sync/types.ts` (10 files) stay
app-side — the frontend identity/generated-type layer is coupled to the concrete
`Stores.Auth`/`useAuthStore`/`User`/`Permissions`/`SyncEntity`, which a
domain-agnostic framework compiled with empty interfaces cannot name (T-5). This
mirrors the backend identity-pluggability decision (#1). `getBaseURL.ts` +
`getBaseURL.desktop.ts` stay app-side (desktop-overridable via the `@/`-scoped
`localOverridePlugin`) and are injected into the framework transport via
`setBaseUrlResolver` — so the desktop base-URL behavior is preserved exactly.

## Known follow-up (out of the F2 tsc gate)

- The four moved node-test specs (`stores.test`, `store-kit.test`,
  `overrides/{registry,seam}.test`) type-check green but their node-test RUNTIME
  loader (the `@/core/*`→stub alias) is not yet wired for the framework package.
  Deferred — F2's gate is `tsc --noEmit`, and the moved sources are
  relative-imported so no alias is needed at type-check time.
- The desktop runtime base-URL override now flows through `setBaseUrlResolver`
  (registered by the always-imported thin `api-client/index.ts`); the desktop
  Tauri E2E that exercises it lives in Chunk D's scope.
