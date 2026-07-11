# PLAN.md — Desktop UI Override Infrastructure

## Problem

Today the ONLY way to customize any UI for the desktop build is
`localOverridePlugin` — a Vite `resolveId` shadow that swaps a **whole `.tsx`
file**: `desktop/ui/src/<path>` wins over `ui/src/<path>` for any `@/…` import.
To change one element inside a core component you must duplicate the entire
enclosing file into the desktop tree (see `HardwareMonitorButton.tsx`: a 70-line
desktop copy of a 50-line core component to change only the click behavior;
`ProviderGroupAssignmentCard.tsx`: an 11-line `return null` shadowing a 65-line
component). This does not scale to the ~413-component web UI.

## Research conclusion (Phase-1 survey — three parallel surveys)

Override mechanisms split on two axes:
- **Axis 1 — build-time file swap** (whole-module; zero runtime cost). ziee
  ALREADY has this: `localOverridePlugin` + the module blocklist + desktop-only
  modules. Good for wholesale component/page/module replacement.
- **Axis 2 — runtime element injection** (sub-element granularity; needs the
  core component to declare a seam). ziee has **NO** general seam for this: slots
  are append-only (no replace), `data-slot`/`cva` are styling-only, and there is
  no runtime component registry. This is the missing capability.

The honest architectural truth: you cannot override an arbitrary element inside a
core component that declares no seam — without either forking the whole file or
fragile AST interception. So the deliverable is a **cheap, typed, ergonomic seam
primitive + registry** that converts "fork the whole file" into "core declares
ONE small fallback-preserving seam + desktop registers ONE override." Seams are
added **on demand**, when a real desktop divergence needs one.

**Chosen mechanism (see DECISIONS DEC-1): the hybrid.** Keep the existing
file-shadow plugin unchanged for wholesale swaps; ADD a runtime **UI Override
Registry** for in-place sub-element seams, modeled byte-for-byte on the existing
chat `panelRendererRegistry` (`Chat.store.ts:104-150`): a module-level typed
`Map`, `registerOverride<K>()` / `resolveOverride<K>()`, keys typed via
declaration-merging on a base `interface UIOverrides {}` (exactly the `Slots` /
`RegisteredStores` / `PanelRendererMap` idiom).

## Items

- **ITEM-1**: Core override registry at `ui/src/core/overrides/` — a module-level
  `Map<string, ComponentType<any>>` with `registerOverride<K extends keyof
  UIOverrides>(key, component)` and `resolveOverride<K>(key): ComponentType<UIOverrides[K]> | undefined`.
  Type-erased storage on the private edge, precise `UIOverrides[K]` on the public
  edge — mirrors `panelRendererRegistry` / `registerPanelRenderer` in
  `Chat.store.ts:104-127`.
- **ITEM-2**: Seam primitive — `useOverride(key, Fallback)` hook returning
  `resolveOverride(key) ?? Fallback`, plus an `<Override id fallback {...props}/>`
  component wrapper for JSX ergonomics. When nothing is registered it renders the
  Fallback, so the web bundle (which registers nothing) is behaviorally identical
  to today.
- **ITEM-3**: Typed key contract — base `export interface UIOverrides {}` in
  `ui/src/core/overrides/types.ts`; each declared seam augments it via
  `declare module` (value type = the overridable element's props). Keys are
  `keyof UIOverrides`; an unknown key is a compile error. Mirrors the
  declaration-merging used for `Slots` (`core/module-system/types.ts:34`) and
  `PanelRendererMap`.
- **ITEM-4**: Desktop registration entry point — a desktop module whose
  `initialize()` calls `registerOverride(...)` for every desktop seam, running
  during `loadDesktopModules()` in `desktop/ui/src/main.tsx` BEFORE
  `ReactDOM.render` (same pre-render window as `setMultiUserMode(false)`), so an
  override is registered before the core component first renders.
- **ITEM-5**: Exemplar conversion #1 (whole-component seam) — convert
  `HardwareMonitorButton` from a whole-file desktop shadow to a seam: core
  declares `hardware.monitor-button` with `DefaultHardwareMonitorButton` as
  fallback; desktop registers the native-window variant. Deletes the desktop
  shadow file; proves the duplication reduction.
- **ITEM-6**: Exemplar conversion #2 (ELEMENT-level seam — the net-new
  capability) — pick a larger core component and override ONE interior element
  via a seam WITHOUT forking the enclosing file. Candidate chosen at implement
  time from: an action in `LeftSidebar`/`HeaderBarContainer`, or the
  `ProviderGroupAssignmentCard` slot inside its parent. Demonstrates changing one
  element while the rest of the component stays shared.
- **ITEM-7**: Discoverability manifest + lint — `ui/scripts/gen-override-registry.mjs`
  (mirrors `gen-testid-registry.mjs`): scan both trees, emit a core-tree manifest
  (`OVERRIDE_MANIFEST.md` + a generated TS list of declared seam keys), and a
  `--check` mode wired into `npm run check` in BOTH workspaces that FAILS on an
  override registered for a key with no declared seam (dead override) and lists
  every declared seam.
- **ITEM-8**: Gallery coverage — add gallery cells for the converted seam
  surfaces so the web gallery renders the FALLBACK and the desktop gallery renders
  the OVERRIDE; satisfy `check:state-matrix` / `check:gallery-coverage` in both
  workspaces.
- **ITEM-9**: Docs — a concise "Desktop UI Override" section (in the design-system
  docs / CLAUDE.md) documenting the two axes, the seam-on-demand policy, the
  `<module>.<element>` key convention, and WHEN to use file-shadow vs a seam. The
  generated `OVERRIDE_MANIFEST.md` (ITEM-7) is the living index this section
  points at.

## Out of scope (candidate follow-ups — NOT items this round; pull in only on approval)

- `.desktop.tsx` co-located build-time resolution (an axis-1 ergonomic upgrade to
  `localOverridePlugin` so whole-component overrides can live NEXT TO the core
  file instead of mirrored deep in the desktop tree). Whole-file only; does not
  address element-level. Deferred to keep v1 focused on the net-new axis-2 seam.
- Bulk migration of all ~18 existing whole-file desktop overrides to seams
  (mechanical follow-up once the pattern is blessed).
- A codemod that auto-extracts an inline element into a declared seam.

## Files to touch

NEW (core):
- `src-app/ui/src/core/overrides/registry.ts` — the Map + register/resolve.
- `src-app/ui/src/core/overrides/useOverride.ts` — hook.
- `src-app/ui/src/core/overrides/Override.tsx` — `<Override>` component wrapper.
- `src-app/ui/src/core/overrides/types.ts` — base `UIOverrides` interface.
- `src-app/ui/src/core/overrides/index.ts` — barrel; re-exported from `core/index.ts`.
- `src-app/ui/src/core/overrides/*.test.ts(x)` — unit tests.
- `src-app/ui/scripts/gen-override-registry.mjs` — manifest + `--check`.

EDIT (core, additive seam declarations):
- `src-app/ui/src/core/index.ts` — export the overrides barrel.
- `src-app/ui/src/modules/hardware/HardwareMonitorButton.tsx` — declare
  `hardware.monitor-button` seam (fallback = existing markup).
- one larger core component for ITEM-6 (chosen at implement time).
- `src-app/ui/package.json` — wire `gen-override-registry.mjs --check` into `check`.
- gallery files under `src-app/ui/src/dev/gallery/` for ITEM-8.

NEW/EDIT (desktop):
- `src-app/desktop/ui/src/modules/desktop-base/` (or a new `overrides` module) —
  the registration entry point (ITEM-4).
- desktop variant components for the two exemplars (ITEM-5/6).
- DELETE `src-app/desktop/ui/src/modules/hardware/HardwareMonitorButton.tsx`
  (replaced by a registered override).
- `src-app/desktop/ui/package.json` — `gen-override-registry.mjs --check`.
- desktop gallery cells for the OVERRIDE state (ITEM-8).

## Patterns to follow

- **Registry shape → chat panel renderer.** `Chat.store.ts:104-150`
  (`panelRendererRegistry` module-level Map + `registerPanelRenderer<T>` +
  `resolvePanelRenderer` + type-erased storage / precise public edge). The
  override registry is the same shape, one level more general.
- **Typed keys via declaration merging → `Slots`** (`core/module-system/types.ts:34-36`),
  `RegisteredStores` (`core/module-system/types-store.ts`), `PanelRendererMap`.
- **Desktop registration timing → `desktop-loader.ts` + `main.tsx`** (desktop
  modules initialize before first render; `setMultiUserMode(false)` is the
  precedent for a pre-render platform mutation).
- **Manifest generator + `--check` → `gen-testid-registry.mjs`** (scans BOTH
  trees, writes a core-tree generated file, both workspaces run it in `--check`).
- **Whole-file override convention (unchanged) → `localOverridePlugin` +
  existing desktop overrides' header-comment style** (e.g. `LeftSidebar.tsx:1-24`
  importing the core version via the un-intercepted `@ziee/ui-core/*` alias).
- **Module structure → `core/module-system/`** (index/store/types split).

## UI-surface plan checklist

This feature adds **infrastructure**, not a new page/drawer/card. It ships two
exemplar conversions that must not regress their host surfaces:

- **Precedent.** Each converted seam's fallback MUST render byte-identically to
  today's core element (the fallback IS the extracted original markup). The
  desktop override mirrors its sibling's structure/tokens. Divergence beyond the
  intended element is a bug.
- **Scale / cardinality.** No new list/collection is introduced; the registry is
  a small fixed Map keyed by declared seams (tens, not thousands). No paging
  concern.
- **Device size / responsive.** The seams are drop-in replacements at existing
  render points; responsive behavior is inherited from the host surface. Gallery
  coverage (ITEM-8) includes the host surface's existing narrow-viewport state.
- **User-visible progress.** N/A — no ingest/produce surface; the override is a
  synchronous render-time swap.
