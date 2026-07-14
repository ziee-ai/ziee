# Chunk `sdk-desktop-shell` — app-layout + settings scaffold → `@ziee/shell` (CUT manifest)

Lands **B-1**, the deferred FE-7 tail from `sdk-shell` BOUNDARY.md: move the
biggest remaining shell copy-cost surface — `modules/layouts/app-layout/**`
(the sidebar chrome + drag/touch orchestration + the 7 slot definitions) and the
generic settings scaffold — into `@ziee/shell`, **while preserving the DESKTOP
`.desktop.tsx` platform-variant override**.

The tangle (per B-1): `LeftSidebar/SidebarToggleButton/Drawer/HeaderBarContainer.
desktop.tsx` + `SettingsPage.desktop.tsx` are whole-file variants the desktop
`localOverridePlugin` swaps **only for `@/`-prefixed specifiers**. A package's
internal imports are relative, so a shell-owned `AppLayout` importing these leaves
directly would always resolve the WEB variant on desktop — a silent desktop
regression invisible to the web-only `gate:ui`.

## Remediation chosen: **(a) slot/config-seam inversion** (NOT (b))

`@ziee/shell`'s `AppLayout` does NOT own the platform-variant leaves. It exposes
`LeftSidebar` + `SidebarToggleButton` as **injected props** (`AppLayoutProps`).
ziee's app-side `AppLayout.tsx` is the INJECTION SITE — it imports the two leaves
via its own `@/`-prefixed specifiers and passes them in, so `localOverridePlugin`
still swaps them to `.desktop` **at ziee's injection site** (the existing
mechanism untouched). Option (b) — patching `localOverridePlugin` to resolve
`.desktop` inside package `src` — was NOT needed; (a) preserves the override
cleanly (proven on Linux via an xvfb desktop build, see TRANSFORMS D-1).

## 3-BUCKET MAP

### BUCKET A — GENERIC STRUCTURE → `@ziee/shell` (moved; app keeps a shim)
| ziee source (→ shim/wrapper at same `@/` path) | → package dest |
|---|---|
| `modules/layouts/app-layout/AppLayout.tsx` | `src/layouts/AppLayout.tsx` (LeftSidebar+SidebarToggleButton **injected props**; `Stores.AppLayout` via typed seam; `cn`→`@ziee/kit/lib/utils`; `@/…`→relative) |
| `modules/layouts/app-layout/hooks/useWindowMinSize.ts` (pure parts) | `src/hooks/useWindowMinSize.ts` (`useWindowMinSize`/`useElementMinSize`/`calculateMinSize`/`applyHysteresis`/`breakpointValues`/types — store-free) |
| `modules/layouts/app-layout/types.ts` (slot type decls) | `src/layouts/appLayoutSlots.ts` (the 4 `Sidebar*Item` ifaces + the `Slots` augmentation; `PermissionExpr`→`@ziee/framework/permissions`) |
| `modules/settings/components/SettingsPageContainer.tsx` | `src/settings/SettingsPageContainer.tsx` (`Stores.AppLayout.nativeScroll` via typed seam; `DivScrollY`→relative) |

`@ziee/shell/src/index.ts` re-exports all four (+ the min-size helper set + the
slot item types).

### BUCKET B — INJECTION SITES / SHIMS (stay app-side, rewritten thin)
- `modules/layouts/app-layout/AppLayout.tsx` → **injection wrapper**: imports
  `LeftSidebar` + `SidebarToggleButton` via `@/` and renders `<ShellAppLayout
  LeftSidebar={…} SidebarToggleButton={…}>`. This is where the desktop swap fires.
- `modules/layouts/app-layout/types.ts` → re-exports the 4 item types from
  `@ziee/shell/layouts/appLayoutSlots` (pulls the `Slots` augmentation into the
  app graph) + KEEPS the app-side `RegisteredStores.AppLayout` store-type augment
  (references the app's concrete `useAppLayoutStore`).
- `modules/layouts/app-layout/hooks/useWindowMinSize.ts` → re-exports the pure
  hooks from shell + KEEPS `useMainContentMinSize` (store-coupled) locally,
  composing shell's exported `calculateMinSize`/`applyHysteresis`.
- `modules/settings/components/SettingsPageContainer.tsx` → byte-thin re-export
  → `@ziee/shell/settings/SettingsPageContainer` (its 35 `@/` importers unchanged).

### BUCKET C — STAYS app-side (platform variants / not generic — NOT moved)
- **The 4 platform-variant leaves + their variants:** `LeftSidebar(.desktop)`,
  `SidebarToggleButton(.desktop)`, `Drawer(.desktop)`, `HeaderBarContainer(.desktop)`.
  Injected (the first two) or standalone app-wide primitives (Drawer 40 `@/`
  consumers, HeaderBarContainer 14) — moving them would break the desktop `@/`
  swap for every consumer (see Decision D-4). Untouched.
- **`SettingsPage(.desktop)` + `SettingsLayout`** — the whole-file `.desktop`
  divergence (single-admin flat menu) is a genuine app override, and the web
  `SettingsPage` is ziee-specific (repo URLs, onboarding, RBAC filtering) with a
  hard dep on the un-movable `HeaderBarContainer`. STAY app-side; the desktop
  override is preserved by `SettingsLayout`'s untouched `@/modules/settings/
  SettingsPage` swap seam (Decision D-3).
- **`AppLayout.store.ts`, `module.tsx`, `index.ts`, `useNativeScroll.ts`,
  `ResizeHandle.tsx`, `SidebarHeaderSpacer.tsx`, `blank/*`** — store is an
  app-registered seam (shell reads it via cast, mirrors DivScrollY); the rest are
  store-coupled or already-moved. Untouched.

## DESKTOP OVERRIDE PRESERVED — proven on Linux (xvfb)
The equivalence check the `sdk-shell` agent could not run. `resolveOverridePath`
(the plugin's pure resolver) resolves all injection specifiers to `.desktop.tsx`;
an actual `xvfb-run vite build` of `desktop/ui` bundles the **desktop** variants
and excludes the **web** ones (marker grep in TRANSFORMS §xvfb). ziee's UI renders
identically on web AND desktop.

## NO Rust / OpenAPI / generated-`types.ts` impact
Pure frontend. `git status` shows zero changes to any `api-client/types.ts`,
`openapi.json`, or Rust file (verified). E8 trivially byte-identical.
