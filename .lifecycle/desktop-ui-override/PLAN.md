# PLAN.md — Desktop UI Override Infrastructure

## Problem

Today the ONLY way to customize UI for the desktop build is `localOverridePlugin`
— a Vite `resolveId` shadow that swaps a **whole `.tsx` file**
(`desktop/ui/src/<path>` wins over `ui/src/<path>` for any `@/…` import). To
change one element you duplicate the entire enclosing file. The triage of the 18
existing overrides quantifies the waste: e.g. `Drawer.tsx` duplicates ~145 lines
of core to change a header block; `SettingsPage.tsx` ~150; `HardwareMonitorButton`
~25 to change one handler.

## Research conclusion (Phase-1 surveys + triage)

Override mechanisms split on two axes; ziee needs BOTH:
- **Axis 1 — build-time file swap** (whole module, zero runtime cost). ziee ships
  this (`localOverridePlugin` + module blocklist + desktop-only modules).
- **Axis 2 — runtime element injection** (sub-element; needs the core component to
  declare a seam). ziee has NONE — slots are append-only, `data-slot`/`cva` are
  styling-only, no component registry exists.

The honest truth: an element with no declared seam can't be overridden without
forking the file. So the deliverable converts "fork the whole file" into "core
declares ONE fallback-preserving seam + desktop registers ONE override."

**Triage of the 18 existing shadows (class A/B/C):**
- **B — element-level (5):** `Drawer`, `SettingsPage`, `HardwareMonitorButton`,
  `SidebarToggleButton`, `SidebarHeaderSpacer` → **convert to seams**.
- **A — whole-file/structural (5):** `AuthGuard`, `LeftSidebar` (zero-dup wrapper),
  `HeaderBarContainer`, `memory/module` (a different module), `ProviderGroupAssignmentCard`
  (a `return null` stub) → **relocate to `.desktop.tsx` co-location** (a seam
  removes nothing here).
- **C — infrastructure (8):** entry/loader/api-client/store/css → **leave as-is**
  (not JSX; a `<Seam>` doesn't apply).

## Chosen design (DECISIONS DEC-1, DEC-12, DEC-13 — approved aggressive)

1. **Runtime UI Override Registry** — modeled on chat `panelRendererRegistry`
   (`Chat.store.ts:104-150`): module-level typed `Map`, `registerOverride<K>()` /
   `resolveOverride<K>()`, keys typed via declaration-merging on `interface
   UIOverrides {}`.
2. **`<Seam>` wrap-in-place primitive** — `<Seam id props>{fallback children}</Seam>`
   renders the registered override(props) or, if none, its children (the original
   markup). Plus a `useOverride(key, Fallback)` hook for logic-heavy sites.
3. **Full auto-migration codemod** (ts-morph, `ui/scripts/seam-codemod.mjs`):
   `migrate` diffs a desktop shadow vs its core sibling and rewrites core (seam
   wrap) + emits the desktop `registerOverride` + deletes the shadow; `add`
   scaffolds a new seam (decl + registration stub + manifest row). **Codemod
   output is human-reviewed** before commit (DEC-12).
4. **`.desktop.tsx` co-location** for class-A whole-file overrides: extend the
   resolver to probe `<path>.desktop.<ext>` in the CORE tree; relocate the 5
   class-A shadows there; web workspace excludes `**/*.desktop.*` from tsconfig +
   biome (Tauri-importing files must not break the web typecheck/lint) (DEC-13).

## Items

- **ITEM-1**: Core override registry `ui/src/core/overrides/registry.ts` —
  module-level `Map` + `registerOverride<K extends keyof UIOverrides>(key, comp)` +
  `resolveOverride<K>(key): ComponentType<UIOverrides[K]> | undefined`; type-erased
  storage / precise public edge; dev-warn on unknown-key resolve. Mirrors
  `panelRendererRegistry`.
- **ITEM-2**: Seam primitives — `<Seam id props>{fallback}</Seam>` (children are
  the fallback) + `useOverride(key, Fallback)`; render override(props) when
  registered else the fallback. Web (registers nothing) is byte-identical to today.
- **ITEM-3**: Typed keys — base `export interface UIOverrides {}` in
  `core/overrides/types.ts`; each seam augments it via `declare module` (value =
  props type). Unknown keys are compile errors. Mirrors `Slots`/`PanelRendererMap`.
- **ITEM-4**: Desktop registration entry point — a desktop module `initialize()`
  invoked by `loadDesktopModules()` in `main.tsx` BEFORE `ReactDOM.render` (same
  window as `setMultiUserMode(false)`), calling `registerOverride(...)` for every
  desktop seam.
- **ITEM-5**: `.desktop.tsx` resolver — extend `plugins/vite-plugin-local-override.ts`
  so, for a desktop `@/foo` import, it probes (order per DEC-14): desktop-tree
  `desktop/ui/src/foo.*` → core-tree `foo.desktop.*` → core-tree `foo.*`. Extract
  the resolution order into a pure function for unit-testing. Update the
  `testidUniquePlugin` scan so `foo.desktop.tsx` is treated as SHADOWING `foo.tsx`,
  not a duplicate-testid collision. Add `**/*.desktop.*` to web `tsconfig.json`
  `exclude` + `biome.json` ignore so Tauri-importing `.desktop` files don't break
  the web workspace typecheck/lint; the desktop workspace keeps them in scope.
- **ITEM-6**: Relocate the 4 class-A component shadows from
  `desktop/ui/src/<path>` to `ui/src/<path>.desktop.tsx` (`AuthGuard`,
  `LeftSidebar`, `HeaderBarContainer`, `ProviderGroupAssignmentCard`), deleting the
  desktop-tree copies. Behavior unchanged; resolved by ITEM-5. (`memory/module`
  reclassified back to a tier-1 desktop-tree module — `module.tsx` is
  glob-discovered and cannot use `.desktop.tsx` resolution; see DRIFT-1.5. The
  desktop auth barrel is updated to re-export AuthGuard via `@/` — the tier-2
  barrel caveat, DRIFT-1.4.)
- **ITEM-7**: Auto-migration codemod `ui/scripts/seam-codemod.mjs` (ts-morph) —
  `migrate <shadow>`: AST-diff the desktop shadow against its core sibling, rewrite
  the core file to wrap the diverging element(s) in `<Seam>`, generate the
  `UIOverrides` declaration, emit the desktop `registerOverride` block, and delete
  the shadow. `add <key>`: scaffold a new seam (decl + registration stub + manifest
  row). Deterministic + fixture-tested; output reviewed before commit.
- **ITEM-8**: Convert the genuinely element-level shadow(s) to seams:
  `HardwareMonitorButton` (done — core declares the seam, desktop registers its
  variant, shadow deleted). On close reading (DRIFT-1.6) `Drawer` + `SettingsPage`
  are STRUCTURAL, not element-level — a seam would need many contorted sub-seams and
  can't capture desktop chrome/inset needs — so both are RETAINED as tier-1
  desktop-tree shadows (like `SidebarToggleButton`/`SidebarHeaderSpacer`/`memory`).
  The seam mechanism's element-level capability is proven by HardwareMonitorButton;
  most existing overrides are honestly structural (file-swap). `HardwareMonitorButton`
  is the codemod's golden fixture.
- **ITEM-9**: Reconcile `Drawer` drift in the retained tier-1 shadow (DRIFT-1.7) —
  RESTORE core's `higherLayerOpen` stacking guard (a real bug: a stacked dialog
  closing also dismissed the drawer) + `titleText`/node-title `Dialog.Title` a11y +
  the `data-testid` override + the `data-slot`/`layout-drawer-content` markers the
  guard's query depends on. Swipe-to-close deliberately NOT ported (low value on a
  mouse-driven desktop). Guard decision logic extracted to `isHigherLayerPresent`
  for unit testing.
- **ITEM-10**: Manifest + lint — `ui/scripts/gen-override-registry.mjs` (or a
  `seam check` subcommand): emit `OVERRIDE_MANIFEST.md` + a generated key list;
  `--check` FAILS on (a) a registered override whose key has no declared seam (dead
  override) and (b) an orphaned `*.desktop.tsx` with no core sibling. Wire
  `--check` into `npm run check` in BOTH workspaces (mirrors `gen-testid-registry`).
- **ITEM-11**: Gallery coverage — gallery cells so the web gallery renders each
  converted seam's FALLBACK and the desktop gallery renders its OVERRIDE; the
  relocated class-A `.desktop.tsx` render in the desktop gallery. Satisfy
  `check:state-matrix` / `check:gallery-coverage` in both workspaces.
- **ITEM-13**: MIGRATE EVERY POSSIBLE raw shadow to its finest mechanism (not just
  the demo). SidebarHeaderSpacer → `<Seam>` (2nd element-level seam); SettingsPage,
  Drawer, SidebarToggleButton → co-located `.desktop.tsx`; loader, App.store,
  lazyWithPreload, getBaseURL → co-located `.desktop.ts` (behavior-preserving
  tier-1→tier-2 relocations, all `@/`-imported); delete the dead auth/index.ts
  barrel. Result: 2 seams + 11 `.desktop` co-locations; only 3 raw shadows remain,
  each an approved SHADOW-EXCEPTION (DEC-17: main.tsx entry, memory/module.tsx glob,
  api-client/types.ts generated).
- **ITEM-14**: RAW-SHADOW GATE — `gen-override-registry.mjs --check` enumerates
  every desktop-tree file with a `src-app/ui` sibling and FAILS on any that is not
  a `<Seam>` registration / a co-located `.desktop.tsx` (no desktop-tree file) / a
  desktop-exclusive module / an approved `SHADOW-EXCEPTION`. Wired into `npm run
  check` in both workspaces; the manifest lists exclusives + exceptions. Proves the
  migration complete AND blocks a future raw whole-file override from sneaking in.
- **ITEM-12**: Docs — a "Desktop UI Override" section (design-system docs /
  CLAUDE.md): the two axes, the `<Seam>`-vs-`.desktop.tsx` decision rule, the
  `<module>.<element>` key convention, and codemod usage; the generated
  `OVERRIDE_MANIFEST.md` is the living index.

## Files to touch

NEW (core): `ui/src/core/overrides/{registry.ts,useOverride.ts,Override.tsx,types.ts,index.ts}`
+ their `*.test.ts(x)`; `ui/scripts/seam-codemod.mjs` (+ `.test.mjs` + fixtures);
`ui/scripts/gen-override-registry.mjs` (or fold into seam-codemod `check`).

EDIT (core): `ui/src/core/index.ts` (export overrides barrel);
`plugins/vite-plugin-local-override.ts` (`.desktop.*` probe + pure resolver fn);
`plugins/vite-plugin-testid-unique.js` (`.desktop` shadow awareness);
web `tsconfig.json` + `biome.json` (`**/*.desktop.*` exclude);
`ui/package.json` + `desktop/ui/package.json` (`--check` wiring);
the 5 class-B core components (seam wraps); gallery files under
`ui/src/dev/gallery/` + `desktop/ui/src/dev/gallery/`.

RELOCATE (git mv, class-A): `desktop/ui/src/{modules/auth/AuthGuard.tsx,
modules/layouts/app-layout/components/LeftSidebar.tsx, .../HeaderBarContainer.tsx,
modules/memory/module.tsx, modules/llm-provider/components/ProviderGroupAssignmentCard.tsx}`
→ `ui/src/<same-path>.desktop.tsx`.

DELETE (class-B shadows, replaced by seams): the 5 desktop-tree copies of the
class-B files.

NEW/EDIT (desktop): the registration entry module (ITEM-4); desktop seam variant
components for the 5 class-B conversions; desktop gallery cells.

## Patterns to follow

- **Registry → chat panel renderer** (`Chat.store.ts:104-150`).
- **Typed keys → `Slots`** (`core/module-system/types.ts:34`) / `PanelRendererMap`.
- **Desktop registration timing → `desktop-loader.ts` + `main.tsx`** (pre-render
  init; `setMultiUserMode(false)` precedent).
- **Resolver edit → the existing `localOverridePlugin` extension-probe loop**
  (`vite-plugin-local-override.ts:42-78`) — add `.desktop.*` as a probe tier.
- **Manifest + `--check` → `gen-testid-registry.mjs`** (scan both trees, core
  generated file, both workspaces `--check`).
- **Codemod → ts-morph** (already a devDep? verify in Phase 5; else the repo's
  existing `.mjs` script + a lightweight AST lib). Fixtures mirror the
  `dev/gallery/__detector_fixtures__` style.
- **Module structure → `core/module-system/`** (index/store/types split).

## UI-surface plan checklist

Infrastructure feature; the risk surface is the 5 converted class-B host
surfaces + the 5 relocated class-A surfaces.

- **Precedent.** Every seam's fallback renders byte-identically to today's core
  element; each desktop variant mirrors its current shadow's behavior (minus
  duplication). Divergence beyond the intended element is a bug (caught by the
  precedent-fidelity audit angle in Phase 6). Drawer is the exception BY DESIGN —
  its conversion FIXES the dropped swipe/stacking (ITEM-9).
- **Scale / cardinality.** No new list; the registry is a small fixed Map (tens of
  seams). No paging concern.
- **Device size / responsive.** Seams are drop-in at existing render points;
  responsive behavior is inherited from the host. Gallery coverage (ITEM-11)
  includes each host's existing narrow-viewport (390px) state — critical for
  `Drawer`/`SettingsPage`/the sidebar components which are layout-sensitive.
- **User-visible progress.** N/A — synchronous render-time swaps.
