# Chunk `sdk-shell` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form, and why. The
theme toggle/accent apply, the error-boundary two-layer isolation, the lazy-type
cache, the meta-theme-color rAF timing, the blank-layout landmark, and the
per-module error-wrapped ordered render are **byte-for-byte preserved**; the
transforms are the mechanical de-coupling (generic type, app-store → typed seam,
`@/` → relative, bootstrap → prop) the package boundary requires.

## Permission primitives → `@ziee/framework/permissions`

- **T-1 `Permission` → `string`; `User` → `PermissionUser`**
  (`permissions/types.ts`, `hasPermission.ts`, `evaluatePermission.ts`). The leaf
  was `Permission = Permissions` (ziee's generated enum) and the evaluator took
  `User` from `@/api-client/types`. Framework can't import the app's generated
  types, so the leaf widens to `string` and the user narrows to the minimal
  `PermissionUser = { is_admin?: boolean | null }` (all the primitives read off
  the user). **Equivalence:** widening an INPUT leaf to `string` only accepts
  MORE (enum members are assignable to `string`); a full `User` is structurally
  assignable to `{ is_admin? }`. The app-side shim `core/permissions/types.ts`
  **re-narrows** `Permission = Permissions` so ziee keeps enum-level type-safety.
  The `is_admin` bypass + `*` + hierarchical `a::b::*` wildcard match are copied
  byte-for-byte. **Decision D-1 (resolved):** widen the leaf (not parameterize
  `PermissionExpr<T>`) — every ziee usage of `PermissionExpr` is an input slot
  (`permission?: PermissionExpr`), so a string leaf is the minimal, exact carrier
  and needs no generic threading through slots.

- **T-2 auth read: `@/modules/auth/Auth.store` + `Stores.Auth` → `authStoreProxy()`
  typed seam** (`permissions/authView.ts`, `usePermission.ts`, `hasPermissionNow.ts`).
  `usePermission` read `Stores.Auth` (proxy, typed via the app's `RegisteredStores`
  merge — invisible to a standalone framework tsc) and `hasPermissionNow` read the
  RAW `useAuthStore.getState()`. Both now go through
  `authStoreProxy() = (Stores as { Auth: StoreProxy<PermissionAuthView> }).Auth`.
  **Equivalence (PROVEN):** `useAuthStore = Auth.store` and `Stores.Auth` is the
  proxy over that same store, so `Stores.Auth.$` ≡ `useAuthStore.getState()`
  (snapshot) and `Stores.Auth.{user,permissions}` reactive-reads are identical to
  the pre-move reads. `usePermission` keeps its reactive read (destructure off the
  proxy = subscribe); `hasPermissionNow` keeps its non-reactive read (`.$`).
  **Decision D-2 (resolved):** a typed local CAST on the framework's own `Stores`
  (not an injected callback / a `configurePermissions` DI) — the runtime read is
  byte-identical to the old code, and a cast can't violate rules-of-hooks the way
  a swappable injected hook could. Seam contract: the app registers a store named
  `Auth` exposing `{ user, permissions }` (ziee's `defineStore('Auth', …)`).

## Shell → `@ziee/shell`

- **T-3 `ThemeProvider`: `Stores.ConfigClient` → typed seam** (`theme/ThemeProvider.tsx`).
  Read `{ themePreference, accentPreset, setThemePreference }` through
  `themeConfig() = (Stores as { ConfigClient: StoreProxy<ThemeConfigView> }).ConfigClient`
  instead of the app's concrete config store. Same store at runtime → the theme
  toggle, `applyAccent`, `<Toaster/>`+`<DialogHost/>` mount are unchanged.
  `ThemePreference` is now defined in `theme/useTheme.ts` (was imported from
  `@/modules/config-client`); the two are the identical literal union so the seam
  type-checks both ways.

- **T-4 `DivScrollY`: `Stores.AppLayout` → typed seam** (`components/DivScrollY.tsx`).
  `const { nativeScroll } = Stores.AppLayout ?? {}` → the same read through a
  `(Stores as { AppLayout?: { nativeScroll? } }).AppLayout ?? {}` cast. The
  store-less fallback (`?? {}`, for the gallery / pre-registration mount) is
  preserved verbatim.

- **T-5 `usePrefetchModules`: `Stores.Routes` → typed seam** (`hooks/usePrefetchModules.ts`).
  Reads the app-registered `Routes` store (populated by `@ziee/framework/router`
  or an app-local router module) through a `{ Routes: { routes: PrefetchRoute[] } }`
  cast, so the shell doesn't need the router augmentation imported at type-check
  time. The idle-callback prefetch of function-form route elements is verbatim.

- **T-6 `App.tsx` bootstrap body → `AppShell` (prop-injected authStore)**
  (`bootstrap/AppShell.tsx`). The `ConditionalComponent` (`shouldMount` gate), the
  `order`-sorted module render, the per-module `<AppErrorBoundary>`, the
  `data-testid="app-root"` div, the `ThemeProvider` wrap, and `initSync` wiring are
  copied verbatim. The only change: `initSync(useAuthStore)` → `initSync(authStore)`
  where `authStore: Parameters<typeof initSync>[0]` is a prop. **why:**
  `loadModules()` (Vite glob, app-owned) + the concrete auth store are app-specific;
  the shell body is generic. ziee's thin `App.tsx` calls `loadModules()` at
  module-eval + renders `<AppShell authStore={useAuthStore}/>` — same boot order.

- **T-7 relative-import rewire (verbatim bodies)** — `theme/themeColor.ts`,
  `theme/ThemeProvider.tsx`, `layouts/BlankLayout.tsx`, `components/LazyComponentRenderer.tsx`
  have their intra-shell `@/…` imports rewritten to package-relative (`./useTheme`,
  `./resolveTheme`, `./accentPresets`, `../theme/themeColor`, `./Loading`). No logic
  change.

## Package plumbing (NEW)

- `sdk/packages/shell/{package.json,tsconfig.json,src/env.d.ts,src/index.ts}` —
  mirror the gallery package (src-export, `.`+`./*` exports, `paths` for
  `@ziee/{kit,framework}`, `declare module '*.css'`). `@ziee/framework` gains a
  `"./permissions"` export. Both `ui/tsconfig.json` and `desktop/ui/tsconfig.json`
  gain `@ziee/shell` + `@ziee/shell/*` path mappings.

## Decision Resolution (zero TBD)
- **D-1 permission leaf** — RESOLVED: widen to `string`; app-shim re-narrows to
  `Permissions`. (Not `PermissionExpr<T>` — every usage is an input slot; T-1.)
- **D-2 auth/config/layout store reads** — RESOLVED: typed local CAST on the
  framework's own `Stores` (byte-identical runtime read), NOT an injected DI hook
  (rules-of-hooks safety + zero behavior change). Seam contract documented in each
  file + `@ziee/shell/index.ts`. (T-2/T-3/T-4/T-5.)
- **D-3 App bootstrap** — RESOLVED: `AppShell` takes `authStore` as a prop;
  `loadModules()` stays app-side (Vite glob can't cross the package boundary). (T-6.)
- **D-4 app-layout + settings scaffold** — RESOLVED (DEFERRED): NOT moved — their
  `.desktop.tsx` platform variants are resolved only by the `@/`-scoped desktop
  `localOverridePlugin`; a package-internal relative import would silently drop the
  desktop override. Documented cut-line + the two remediation options in BOUNDARY
  B-1. `blank` (desktop-variant-free) moved.
- **D-5 router** — RESOLVED: out of scope — already `@ziee/framework/router`.
