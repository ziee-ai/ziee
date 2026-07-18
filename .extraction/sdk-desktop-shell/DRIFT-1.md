# Chunk `sdk-desktop-shell` — DRIFT round 1

Drift = any behavioural divergence between the pre-move ziee app-layout/settings
scaffold and the extracted `@ziee/shell` structure + app shims/injection sites,
checked symbol-by-symbol against the pre-move files and confirmed by the live
gallery runtime-health run, all 3 tsc gates, AND the xvfb desktop build.

- **DRIFT-1.1 (platform override)** — verdict: none. The desktop `.desktop.tsx`
  variants still win. `resolveOverridePath` → `.desktop` for `LeftSidebar`,
  `SidebarToggleButton`, `SettingsPage`; the xvfb desktop build bundles the
  desktop-variant-unique markers (`desktop-mac-glass-sidebar`, `data-sidebar-mask`,
  `Open sidebar`, `size-7 min-w-7`, `Settings sections`) and excludes the
  web-variant-unique ones (`aria-expanded:bg-transparent`, `z-[35]`, `h-[40px]`,
  `Settings navigation`, `settings-mobile-dropdown-*`). Remediation (a) preserved
  the override with no plugin change.
- **DRIFT-1.2 (AppLayout behavior)** — verdict: none. The sidebar drag/resize
  (MIN/MAX/COLLAPSED widths, SIDEBAR/SPACER transition strings, imperative
  drag-time writes), the xs mobile Sheet (open-guard, swipe-to-close/open, mask),
  the visual-viewport keyboard heuristic, the ResizeObserver main-content width,
  the skip link, and the `appBanners` render are all byte-copied. Only the store
  reads became a typed seam cast (same live proxy → same reactivity) and the two
  leaves became injected props (same components at the same render slots).
- **DRIFT-1.3 (store reactivity)** — verdict: none. `appLayout` = the SAME
  `Stores.AppLayout` proxy; destructured `isSidebarCollapsed`/`nativeScroll` reads
  subscribe, `.$ .sidebarWidth` is the snapshot, action calls dispatch identically.
- **DRIFT-1.4 (min-size hooks)** — verdict: none. `useWindowMinSize`/`useElementMinSize`
  + `calculateMinSize`/`applyHysteresis`/`breakpointValues` moved verbatim (only
  `applyHysteresis` promoted to exported). `useMainContentMinSize` unchanged
  app-side, composing shell's helpers — identical hysteresis + subscribe.
- **DRIFT-1.5 (slot types)** — verdict: none. The 4 `Sidebar*Item` ifaces + the
  `Slots` augmentation are byte-copied; `PermissionExpr` swapped to the identical
  framework type. The augmentation reaches every app-side registrar via the
  re-export + side-effect import in the app `types.ts` shim. `RegisteredStores.
  AppLayout` store-type augment stayed app-side.
- **DRIFT-1.6 (SettingsPageContainer)** — verdict: none. Title/subtitle layout,
  the max-w-4xl centering, the native-scroll flow vs DivScrollY branch, and the
  safe-area bottom inset are byte-copied; only the `nativeScroll` read is a seam
  cast (false fallback) and `DivScrollY` is a relative import. 35 importers keep
  the shim.
- **DRIFT-1.7 (settings/Drawer/HeaderBarContainer left in place)** — verdict: none.
  SettingsPage(+desktop), SettingsLayout, Drawer(+desktop), HeaderBarContainer
  (+desktop), the store, useNativeScroll, ResizeHandle, SidebarHeaderSpacer are
  untouched → their behavior is trivially unchanged.
- **DRIFT-1.8 (render identity, live)** — verdict: none. Gallery runtime-health
  over 200 surface/state cells ×2 themes reported **0 gating HIGH findings** — the
  app-layout/settings surfaces (which wrap every page) render healthy on web.

**Unresolved drifts:** 0
