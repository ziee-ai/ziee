# Chunk F2 — `@ziee/framework` core runtime + ApiClient runtime + sync client — CUT manifest

Move ziee's domain-agnostic frontend RUNTIME — the meta-framework core
(`ui/src/core/*`: module system, `createModule`, EventBus, Stores proxy +
store-kit, the desktop UI-override seam, the realtime sync CLIENT) and the
ApiClient transport (`api-client/core.ts` + `sse-types.ts` + the
`createApiClient` factory) — into `sdk/packages/framework/src/` as the
`@ziee/framework` npm workspace package, **equivalence-preserving**. Runtime
behavior is unchanged; only import specifiers + a generated-types INTERFACE
indirection change. ziee `ui/` + `desktop/ui/` then author against
`@ziee/framework`.

## Design gate — the framework is domain/identity-agnostic + compiles standalone

The framework must (a) name **zero** app types/stores/generated files, (b)
compile standalone (`tsc --noEmit` = 0) with the augmentable interfaces
(`RegisteredStores`/`AppEvents`/`Slots`/`UIOverrides`/`CreateModuleOptions`)
**empty**, and (c) consume the app's generated types
(`ApiEndpoints`/`SyncEntity`/`SyncAction`) via a FRAMEWORK-SIDE interface the
per-app `types.ts` satisfies — never ziee's concrete generated file.

**Gate (wire-irrelevant → tsc-clean, not the openapi golden):** `@ziee/framework`
builds standalone (`tsc --noEmit` = 0); ziee `ui/` + `desktop/ui/` `tsc --noEmit`
= 0. No route/type/schema is touched (generated `types.ts` byte-identical to
baseline), so the E8 openapi/types golden does not apply.

## Files — MOVED INTO `sdk/packages/framework/src/` (submodule `sdk/`)  — 28 files

Meta-framework core (`ui/src/core/*` → framework `src/*`), bodies moved
byte-preserved; only `@/…` import specifiers rewritten framework-relative:

- `module-system/{index,types,store,types-store}.ts` (4) — the module registry
  + `Slots` interface + `RegisteredStores.ModuleSystem` augmentation.
- `events/{index,types,store,types-store}.ts` (4) — the EventBus + `AppEvents`
  interface + `RegisteredStores.EventBus` augmentation.
- `overrides/{index,types,registry,Override,useOverride}.ts[x]` + `registry.test.ts`
  + `seam.test.ts` + `OVERRIDE_MANIFEST.md` (8) — the desktop UI-override seam +
  `UIOverrides` interface. (Already relative-imported — byte-preserved.)
- `module.ts` (1) — `createModule` + the augmentable `CreateModuleOptions`.
- `stores.ts` + `stores.test.ts` (2) — `createStoreProxy` + `Stores` proxy +
  `RegisteredStores`/`StoreProxy`.
- `store-kit.ts` + `store-kit.test.ts` (2) — `defineStore`/`defineLocalStore`/
  `defineExtensionStore`.
- `sync/{SyncClient,connection,index}.ts` (3) — the realtime sync CLIENT +
  connection-id holder + `initSync` (DI over an auth-store-like).
- `__test-stubs__/{events,module-system}.ts` (2) — node-test boundary stubs.
- `api-client/core.ts` (1) — the transport: auth token, silent 401-refresh,
  retry, SSE parse, file-upload progress. `callAsync` genericized (T-2).
- `api-client/sse-types.ts` (1) — the SSE handler/callback types (already
  domain-free; byte-preserved).

## Files — NEW in the framework (package infra + the type-boundary seams)

- `src/index.ts` — the top barrel (`export *` of module-system/module/stores/
  events/overrides — the same surface as the old `core/index.ts`).
- `src/api-client/index.ts` — the generic `createApiClient<TClient>(endpoints)`
  factory (the old `index.ts` runtime loop, type-erased) + transport re-exports
  (`callAsync`, `getAuthToken`, `getBaseUrl`, `setBaseUrlResolver`,
  `setUnauthorizedHandler`, SSE types). See TRANSFORMS T-2.
- `src/env.d.ts` — ambient `import.meta.env` shim (the runtime reads
  `import.meta.env.DEV`; the framework has no direct Vite dep). T-6.
- `package.json` — `@ziee/framework`: `exports` (`.` barrel + `./api-client` +
  `./*`), react/react-dom peers, zustand+immer deps, `@types/*` devDeps.
- `tsconfig.json` — bundler resolution, strict, react-jsx, node/react types
  (mirrors `@ziee/kit`).

## Files — CHANGED IN ziee (submodule `src-app/`, NOT committed here)

- **del:** 29 files from `ui/src/core/*` + `ui/src/api-client/{core,sse-types}.ts`
  (the moved slice).
- **edit:** ~600 files under `ui/src/` + `desktop/ui/src/` — import specifier
  `@/core/…` → `@ziee/framework/…` and `@/api-client/{core,sse-types}` →
  `@ziee/framework/api-client/…` (quote-anchored), incl. `declare module`
  augmentation specifiers (both `@/core/*` and relative `../../core/stores`
  forms).
- **rewrite:** `ui/src/api-client/index.ts` — now the thin per-app BINDING of
  the framework factory to this app's generated types (T-2).
- **rewrite:** `ui/src/index.ts` (the `@ziee/ui-core` barrel) — core re-exports
  repointed at `@ziee/framework`.
- **edit:** `ui/package.json`, `desktop/ui/package.json` — add
  `"@ziee/framework": "*"` workspace dep.
- **edit:** `ui/tsconfig.json`, `desktop/ui/tsconfig.json` — add
  `@ziee/framework` + `@ziee/framework/api-client` + `@ziee/framework/*` `paths`.

## Stays app-side (each app owns) — 10 core files + the api-client per-app layer

The identity/generated-type-coupled subset CANNOT cleanly cross into a
domain-agnostic framework (TRANSFORMS T-5):

- `ui/src/core/permissions/*` (7) — reads the concrete `Stores.Auth`, imports the
  concrete `useAuthStore` (`@/modules/auth`), and types over the generated
  `User`/`Permissions`. Moving it would break the framework's standalone `tsc`
  (empty `RegisteredStores` → `Stores.Auth` type error). The frontend identity
  layer stays app-side (the analog of backend identity-pluggability, decision #1).
- `ui/src/core/components/{Loading,LazyComponentRenderer}.tsx` (2) — generic view
  helpers, out of the "runtime" scope; keeping them app-side keeps the framework's
  dep surface minimal (react/zustand only, no `@ziee/kit`).
- `ui/src/core/sync/types.ts` (1) — DERIVES the `sync:<entity>` `AppEvents` map
  from the app's generated `SyncEntity` union. This IS the framework-side-interface
  boundary for sync: it augments `@ziee/framework/events` with the app's concrete
  entities. Per-app OUTPUT-adjacent.
- `ui/src/api-client/{index.ts,types.ts,getBaseURL.ts,getBaseURL.desktop.ts}` —
  `types.ts` generated (per-app OUTPUT, byte-identical to baseline); `index.ts` the
  thin type-binding; `getBaseURL*` the platform base-URL impls (desktop-overridable
  via the `@/`-scoped `localOverridePlugin`) injected into the framework via
  `setBaseUrlResolver` (T-4).
