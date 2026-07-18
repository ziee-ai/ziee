# Chunk `sdk-desktop-shell` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form, and why. The
sidebar drag/resize/collapse logic, the mobile Sheet + swipe gestures, the
visual-viewport keyboard heuristic, the `appBanners` slot render, the breakpoint
hysteresis, and the SettingsPageContainer native-scroll branch are **byte-for-byte
preserved**; the transforms are the mechanical de-coupling (inject the platform
leaves, app-store → typed seam, `@/` → relative/package) the package boundary +
the desktop-override constraint require.

## AppLayout structure → `@ziee/shell/layouts/AppLayout`

- **T-1 platform-variant leaves `LeftSidebar` / `SidebarToggleButton`: internal
  `@/`-imports → INJECTED props** (`AppLayout.tsx`). Pre-move, `AppLayout` imported
  both from `@/modules/layouts/app-layout/components/…`; the desktop
  `localOverridePlugin` swapped those to `.desktop.tsx`. A package uses relative
  imports, which the plugin never sees → the WEB leaf would always win on desktop.
  **Fix:** `AppLayoutProps` adds `LeftSidebar: ComponentType` +
  `SidebarToggleButton: ComponentType`; the shell renders `<LeftSidebar/>` /
  `<SidebarToggleButton/>` from props. ziee's app-side `AppLayout.tsx` becomes the
  INJECTION SITE — it imports the two leaves via `@/` (swapped to `.desktop` by
  the plugin at ZIEE's site) and passes them in. **Decision D-1 (resolved):
  remediation (a) slot/config-seam inversion, NOT (b) patching the plugin.** (a)
  keeps the existing override mechanism 100% untouched (the swap still happens at a
  `@/` site, just relocated app-side); (b) would special-case package `src` in a
  build plugin. **PROVEN on Linux (xvfb) — see §xvfb below.**

- **T-2 `Stores.AppLayout` reads → typed seam cast** (`AppLayout.tsx`). The shell
  can't import the app's concrete AppLayout store type. A local `AppLayoutSeam`
  interface + `const appLayout = (Stores as unknown as { AppLayout: AppLayoutSeam
  }).AppLayout` replaces the 12 `Stores.AppLayout.*` accesses. **Equivalence:** the
  cast returns the SAME live proxy, so `isSidebarCollapsed`/`nativeScroll`
  destructures stay reactive (subscribe) and `.$` snapshot / action calls are
  byte-identical. Mirrors the existing `DivScrollY` seam exactly.

- **T-3 mechanical import rewrites** (`AppLayout.tsx`): `cn` `@/lib/utils` →
  `@ziee/kit/lib/utils`; `useMetaThemeColor` `@/components/ThemeProvider/themeColor`
  → `../theme/themeColor` (already in shell); `LazyComponentRenderer`
  `@/core/components/…` → `../components/…` (already in shell); `useWindowMinSize`
  `@/…/hooks/…` → `../hooks/useWindowMinSize`. Runtime unchanged; `Stores.ModuleSystem`
  + `Sheet/SheetContent/SheetTitle` from `@ziee/kit/shadcn/sheet` are untouched.

## min-size hooks → `@ziee/shell/hooks/useWindowMinSize`

- **T-4 hook split: pure → shell, store-coupled stays app-side.**
  `useWindowMinSize` / `useElementMinSize` + `calculateMinSize` / `applyHysteresis`
  / `breakpointValues` / `Breakpoint` / `MinSize` have NO store coupling → moved
  verbatim (`applyHysteresis` promoted from module-private to exported). The app
  shim re-exports them and KEEPS `useMainContentMinSize` (which reads
  `useAppLayoutStore.getState()`/`.subscribe`) locally, composing shell's exported
  helpers. **Decision D-2 (resolved):** split the file (not move the whole thing
  behind a seam) — only one of three hooks touches the store, and its 0 external
  callers today don't justify a store-subscribe seam in the package.

## slot type decls → `@ziee/shell/layouts/appLayoutSlots`

- **T-5 `Slots` augmentation + 4 `Sidebar*Item` ifaces moved; `PermissionExpr`
  `@/core/permissions` → `@ziee/framework/permissions`.** The `declare module
  '@ziee/framework/module-system/types' { interface Slots { … } }` augmentation +
  the item interfaces move to shell. **Equivalence:** `@ziee/framework/permissions`
  exports the SAME `PermissionExpr` structural type; the augmentation is pulled
  into the app's compilation by the app-side `types.ts` re-exporting from (and
  side-effect importing) `@ziee/shell/layouts/appLayoutSlots`, so every app-side
  slot registration + `import '@/modules/layouts/app-layout/types'` side-effect
  (server-update, LeftSidebar, chat/types) sees the slot keys unchanged. The
  app-side `RegisteredStores.AppLayout` store-type augmentation STAYS app-side
  (it references the app's concrete `useAppLayoutStore`).

## settings scaffold → `@ziee/shell/settings/SettingsPageContainer`

- **T-6 `SettingsPageContainer` moved; `Stores.AppLayout.nativeScroll` → seam
  cast; `DivScrollY` → relative.** Verbatim except the two decouplings: the
  `nativeScroll` read uses the `(Stores as … { AppLayout?: { nativeScroll? } })`
  cast with a `false` fallback (store-less gallery safe, mirrors DivScrollY), and
  `DivScrollY` imports from `../components/DivScrollY` (already in shell). Its 35
  app-side importers keep the byte-thin `@/modules/settings/components/
  SettingsPageContainer` shim.

## Decision Resolution (zero TBD)

- **D-1 (resolved):** remediation (a) slot/config-seam inversion for the 2 leaves
  AppLayout renders. Not (b). Proven via xvfb desktop build.
- **D-2 (resolved):** split the min-size hooks; `useMainContentMinSize` stays
  app-side.
- **D-3 (resolved):** `SettingsPage` (both variants) + `SettingsLayout` STAY
  app-side. Rationale: SettingsPage is not generic (ziee repo URLs, onboarding,
  RBAC permission-filtering, the `.desktop` single-admin flat-menu whole-file
  divergence) and has a hard dep on `HeaderBarContainer` (an un-movable app-wide
  platform-variant primitive, D-4). Only the genuinely-generic `SettingsPageContainer`
  moved. The desktop `SettingsPage` override is PRESERVED unchanged — `SettingsLayout`
  imports `@/modules/settings/SettingsPage`, and `resolveOverridePath` confirms that
  specifier still resolves to `SettingsPage.desktop.tsx` on desktop (§xvfb). This is
  the scoped deviation from "SettingsPage generic → shell"; (a) is NOT globally
  infeasible — it is simply not worth moving a non-generic body whose only movable
  dependency (`HeaderBarContainer`) cannot move.
- **D-4 (SUPERSEDED for `Drawer` + `ResizeHandle`; `HeaderBarContainer` still
  app-side):** The original D-4 kept `Drawer`/`HeaderBarContainer` app-side on the
  premise that moving them "would rewrite every consumer's import to a package
  path" → break the `.desktop` swap for 40+ consumers. That premise is FALSE under
  the **shim-at-`@/`** pattern already proven in THIS chunk for `DivScrollY` +
  `useWindowMinSize`: the generic core moves to `@ziee/shell`, but the app keeps a
  thin `@/`-path re-export shim, so **consumers' `@/` imports are unchanged** and
  `localOverridePlugin` still intercepts that `@/` specifier → `.desktop` on the
  Tauri build. So:
  - **`Drawer` MOVED → `@ziee/shell/components/Drawer`** (the core/web body). App
    keeps `@/…/components/Drawer.tsx` as a shim (`export { Drawer, type DrawerProps }
    from '@ziee/shell/components/Drawer'`). `Drawer.desktop.tsx` STAYS app-side
    unchanged — it reaches into `@ziee/desktop/core/platform` + `@tauri-apps/api`
    for window-chrome behavior, so it can't live in the SDK; its `@/` imports of
    `ResizeHandle`/`useWindowMinSize`/`DivScrollY` resolve via their shims. The
    desktop override is byte-identical (same `@/` path, same `.desktop` sibling).
  - **`ResizeHandle` MOVED → `@ziee/shell/components/ResizeHandle`** (dependency-
    pure; no `.desktop` variant). App keeps a shim at `@/…/components/ResizeHandle.tsx`.
  - **`HeaderBarContainer` (+ `.desktop`) still app-side** — a candidate for the
    identical shim treatment in a later chunk, deferred here to keep the move
    scoped to what the human asked for (Drawer + resize handle).
  Both new files are also barrel-exported from `@ziee/shell` (next to `DivScrollY`)
  for discoverability. Deps `@radix-ui/react-dialog` + `react-icons` added to
  `@ziee/shell/package.json` (previously transitive via the app).

## §xvfb — DESKTOP OVERRIDE PROVEN (the equivalence check, run on Linux)

1. **Resolver (the plugin's pure `resolveOverridePath`)** — with `localSrc=desktop/
   ui/src`, `fallbackSrc=ui/src`, `aliasPrefix=@/`:
   - `@/…/components/LeftSidebar` → **LeftSidebar.desktop.tsx**
   - `@/…/components/SidebarToggleButton` → **SidebarToggleButton.desktop.tsx**
   - `@/modules/settings/SettingsPage` → **SettingsPage.desktop.tsx**

2. **Real `xvfb-run npx vite build` of `desktop/ui` (RC=0)** — grep of the emitted
   bundle (`dist/assets/*.js`):

   | marker | source (unique to) | count | expected |
   |---|---|---|---|
   | `desktop-mac-glass-sidebar` (data-source) | LeftSidebar**.desktop** | 1 | present ✓ |
   | `data-sidebar-mask` | LeftSidebar**.desktop** | 1 | present ✓ |
   | `Open sidebar` (Tooltip title) | SidebarToggleButton**.desktop** | 1 | present ✓ |
   | `size-7 min-w-7` (class) | SidebarToggleButton**.desktop** | 1 | present ✓ |
   | `aria-expanded:bg-transparent` | SidebarToggleButton (**web**) | 0 | absent ✓ |
   | `z-[35]` / `h-[40px]` | SidebarToggleButton (**web**) | 0 | absent ✓ |
   | `Settings sections` (aria-label) | SettingsPage**.desktop** | 1 | present ✓ |
   | `Settings navigation` (aria-label) | SettingsPage (**web**) | 0 | absent ✓ |
   | `settings-mobile-dropdown-trigger` … | SettingsPage (**web**) | 0 | absent ✓ |
   | `app-sidebar` / `data-sidebar-resize-handle` | **shell** AppLayout | 5 / 3 | present ✓ |
   | `settings-page-title` | **shell** SettingsPageContainer | 1 | present ✓ |

   The shell's generic AppLayout is bundled and orchestrating; it renders the
   **desktop** leaves on desktop; the **web** leaves + web SettingsPage are
   excluded. The desktop variant is NOT silently replaced by the web one.
