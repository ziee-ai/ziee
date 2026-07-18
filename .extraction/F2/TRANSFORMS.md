# Chunk F2 — TRANSFORMS

Every non-byte-identical change applied while moving the core runtime into
`@ziee/framework`, each with the design decision + resolution. Zero TBD.

## T-1 — moved source is body-preserved; only import specifiers rewritten

The 28 moved files keep their bodies byte-for-byte. The only in-file edits are
import **specifiers** naming the old `@/` locations, rewritten framework-relative:
`@/core/module-system` → `./module-system` / `../module-system`; `@/core/stores`
→ `./stores` / `../stores`; `@/core/events` → `./events` / `../events`;
`@/api-client/sse-types` → `./sse-types`; `@/core/sync/connection` →
`../sync/connection`. The `overrides/*` files were already relative-imported and
are byte-identical. `sse-types.ts`, `connection.ts`, `sync/index.ts` are
byte-identical bar their (already-relative) imports.

**Resolution:** a full-tree grep of `sdk/packages/framework/src` for `from '@/`
/ `import('@/` returns **zero** hits (DRIFT-1.1). Runtime logic is untouched.

## T-2 — the generated-types INTERFACE (ApiClient runtime crosses the boundary)

### Decision — the ApiClient runtime is typed against per-app generated types

`api-client/core.ts::callAsync` was `<U extends ApiEndpointUrl>(url: U, params:
ParameterByUrl<U>): Promise<ResponseByUrl<U>>` and `index.ts::createApiClient`
built a fully-typed `ApiClient` from the generated `ApiEndpoints` map +
`ApiEndpointParameters`/`ApiEndpointResponses` mapped types. All of those live in
the per-app generated `api-client/types.ts`, which STAYS per-app OUTPUT — so the
framework runtime cannot import them, yet it must not lose ziee's per-endpoint
typing at the 133 `ApiClient.*` call sites.

**Resolution:** split runtime (framework, generic) from the type-binding (app,
concrete):

- **Framework** `api-client/core.ts::callAsync` is genericized to
  `<TResponse = unknown>(endpointUrl: string, params: any, …): Promise<TResponse>`
  — the transport body is byte-identical; only the compile-time signature relaxes
  (the runtime never used the concrete parameter type — it inspects strings /
  `FormData`). The `ResponseByUrl<U>` return casts become `TResponse`.
- **Framework** `api-client/index.ts` ships `createApiClient<TClient>(endpoints:
  Record<string, string>): TClient` — the old namespace-grouping loop, type-erased,
  generic over the concrete client type.
- **App** `ui/src/api-client/index.ts` becomes the THIN binding: it keeps the
  per-app `ApiClientType` mapped type (over its generated `ApiEndpoints` /
  `ApiEndpointParameters` / `ApiEndpointResponses`) and does
  `export const ApiClient = createApiClient<ApiClientType>(ApiEndpoints)`. Runtime
  identical; ziee re-derives the exact same type. The generated `types.ts` is the
  interface the framework's generic factory is *satisfied by* — the framework never
  names it.
- **Sync**: `SyncClient.ts` imported the generated `SyncEvent`. The runtime only
  reads `.entity`/`.action`/`.id` (all strings) off each frame, so the framework
  declares a local generic `interface SyncEvent { entity: string; action: string;
  id: string }`. `SyncEntity`/`SyncAction` are never referenced by the runtime;
  the app's `sync/types.ts` (stays app-side) derives the typed `sync:<entity>`
  `AppEvents` map from its generated `SyncEntity` union — the sync half of the
  same interface indirection.

This is the "framework references the shapes generically, the per-app `types.ts`
satisfies them" contract, crossed cleanly. Verified: LEDGER F2-02, F2-07.

## T-3 — the declaration-merge boundary (the audit-flagged fiddly part)

### Decision — apps augment `RegisteredStores`/`AppEvents`/`Slots`/`UIOverrides`/`CreateModuleOptions` across a PACKAGE boundary

These five interfaces are declared in the framework but augmented by every ziee
module via `declare module '@/core/…'`. Post-move the augmentation target must be
a STABLE public specifier that resolves to the file DECLARING the interface (TS
module augmentation cannot target a re-export). Additionally, the framework must
COMPILE STANDALONE with all five empty — where `keyof Slots`/`keyof AppEvents`
collapse to `never` and break code that keys maps / emits events by them.

**Resolution:** three parts, all verified by the framework's own `tsc --noEmit`:

1. **Stable subpath specifiers.** Each interface keeps its declaring file, exposed
   as a subpath via the package `exports` (`"./*": "./src/*"`) + consumer `paths`:
   `RegisteredStores` → `@ziee/framework/stores`, `AppEvents` →
   `@ziee/framework/events` (barrel re-export — the same barrel-augmentation that
   worked pre-move for `@/core/events`), `Slots` →
   `@ziee/framework/module-system/types`, `CreateModuleOptions` →
   `@ziee/framework/module`, `UIOverrides` → `@ziee/framework/overrides`. All ziee
   `declare module '@/core/*'` specifiers (74) AND the relative-path form
   (`declare module '../../core/stores'`, 8 files) are rewritten to these. Grep
   confirms the augmentations now bind — ziee `Stores.Auth`/`Stores.App`/… resolve
   (they failed loudly mid-drift until every augmentation specifier was crossed).
2. **`SlotKey` conditional** (`module-system/store.ts`):
   `type SlotKey = [keyof Slots] extends [never] ? string : keyof Slots`. With the
   empty base `Slots` (framework), `SlotKey = string` so the slot map compiles;
   once an app augments `Slots`, it resolves to that app's exact union — ziee keeps
   its stricter typing unchanged. The `Object.entries(slots)` iteration is cast
   `as Record<string, any[]>` (value-type erasure that was implicit when `Slots`
   was populated). Runtime identical.
3. **Empty-`AppEvents` emit** (`SyncClient.ts`): the `sync:reconnect` emit is cast
   `as never` (matching the pre-existing `as never` on the per-entity emit) so it
   type-checks against an empty `AppEvents`; `as never` is assignable in ziee too,
   so ziee behavior is unchanged.

The boundary crosses cleanly; nothing here is left unresolved. Verified:
LEDGER F2-03, F2-08.

## T-4 — base-URL resolver injected (getBaseURL stays app-side)

### Decision — the base URL is platform-specific and desktop-overridable

`callAsync` + `SyncClient` need a base URL. `getBaseURL.ts` (browser same-origin)
and `getBaseURL.desktop.ts` (Tauri dynamic port) are app/platform impls the
desktop build swaps via the `@/`-scoped `localOverridePlugin` (`@/api-client/
getBaseURL` → `.desktop`). That override mechanism only intercepts `@/`
specifiers, so moving `getBaseURL` into `@ziee/framework` would silently break the
desktop base URL.

**Resolution:** `getBaseURL.ts` + `getBaseURL.desktop.ts` STAY app-side,
unchanged. The framework transport takes the base URL via injection — a
module-level `baseUrlResolver` + `setBaseUrlResolver()`, defaulting to
`window.location.origin` (so a web app that registers nothing still works). The
thin app `api-client/index.ts` (always imported at boot) registers
`setBaseUrlResolver(getBaseUrl)` where `getBaseUrl` is `@/api-client/getBaseURL`
— desktop's `localOverridePlugin` still swaps in the Tauri-port variant, so
desktop behavior is preserved exactly. This is the SAME injection pattern the file
already used for `setUnauthorizedHandler` and that `initSync(authStore)` uses for
the auth store. Verified: LEDGER F2-05.

## T-5 — permissions + components + sync/types stay app-side (reported, per the gate)

### Decision — the identity/generated-type-coupled subset cannot cross cleanly

`core/permissions/*` reads the concrete `Stores.Auth`, imports the concrete
`useAuthStore` (`@/modules/auth/Auth.store`), and types over the generated
`User`/`Permissions`. In a domain-agnostic framework compiled with an empty
`RegisteredStores`, `Stores.Auth` is a type error — the framework cannot hardcode
an identity store. `core/sync/types.ts` derives its event map from the app's
generated `SyncEntity`. `core/components/*` are generic view helpers that would
drag `@ziee/kit` into the framework's dep surface.

**Resolution:** these 10 files STAY in ziee under `@/core/*`, UNCHANGED except the
`@/core/stores`/`@/core/events` import specifiers they consume (rewritten to
`@ziee/framework/*`). This mirrors the backend identity-pluggability decision (#1):
the concrete identity layer is app-owned; the framework enforces generically. This
is the "if it can't be cleanly crossed, report exactly why" the chunk asked for —
reported here + in BOUNDARY. (`core/permissions` still type-checks in ziee because
ziee's `RegisteredStores` IS augmented with `Auth`.)

## T-6 — package infra (env shim, exports + consumer paths)

### Decision — `import.meta.env` + extensionless subpaths under standalone tsc

The runtime reads `import.meta.env.DEV` (Vite), but the framework has no direct
Vite dep; and deep subpaths (`@ziee/framework/module-system/types`) are
extensionless, which `tsc` does not resolve through an `exports` wildcard (same
finding as F1 T-6).

**Resolution:** `src/env.d.ts` declares the minimal `import.meta.env` surface for
the package's own `tsc`; the consuming app supplies the real `vite/client`. The
package keeps `exports` (`.` + `./api-client` + `./*`) for runtime bundlers, and
BOTH `ui/tsconfig.json` + `desktop/ui/tsconfig.json` gain `@ziee/framework` +
`@ziee/framework/api-client` + `@ziee/framework/*` `paths` (the established
`@ziee/kit` pattern). Verified: LEDGER F2-10.

## T-7 — query-param value coerced to string

### Decision — `callAsync(params: any)` relaxes the GET query-encode value type

With `params` relaxed to `any` (T-2), `encodeURIComponent(value)` in the GET
query loop no longer narrows to a primitive.

**Resolution:** `encodeURIComponent(String(value))`. `encodeURIComponent` already
ToStrings its argument, so `encodeURIComponent(value) === encodeURIComponent(
String(value))` for all `value` — runtime-identical, and it type-checks
standalone. Verified: LEDGER F2-01.
